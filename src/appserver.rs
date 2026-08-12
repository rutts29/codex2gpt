use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::audit::redact_for_log;
use crate::error::{AppError, Result};

type RpcResult = std::result::Result<Value, AppError>;

#[derive(Clone, Debug)]
pub struct AppServerClient {
    binary: PathBuf,
    state_dir: PathBuf,
    session: Arc<Mutex<Option<AppServerSession>>>,
    waiters: Arc<Mutex<HashMap<u64, mpsc::Sender<RpcResult>>>>,
    pending_requests: Arc<Mutex<HashMap<String, Value>>>,
    next_id: Arc<AtomicU64>,
    initialized: Arc<AtomicBool>,
    init_lock: Arc<Mutex<()>>,
}

#[derive(Debug)]
struct AppServerSession {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
}

impl AppServerClient {
    pub fn new(binary: impl Into<PathBuf>, state_dir: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            state_dir: state_dir.into(),
            session: Arc::new(Mutex::new(None)),
            waiters: Arc::new(Mutex::new(HashMap::new())),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(false)),
            init_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn ensure_initialized(&self) -> Result<()> {
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        let _guard = self.init_lock.lock().map_err(|_| {
            AppError::CodexCommand("failed to lock app-server init mutex".to_owned())
        })?;
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }

        let _ = self.call_raw(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "codex2gpt",
                    "title": "codex2gpt",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": {
                    "experimentalApi": true
                },
            }),
        )?;

        self.send_notification("initialized", json!({}))?;
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.ensure_initialized()?;
        self.unwrapped_call(method, params)
    }

    pub fn events_for_thread(&self, thread_id: &str) -> Result<Vec<Value>> {
        let path = self.event_log_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let raw = fs::read_to_string(&path).map_err(|source| AppError::ReadFile {
            path: path.clone(),
            source,
        })?;
        let mut events = Vec::new();

        for line in raw.lines() {
            let Ok(entry) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if event_thread_id(&entry) == Some(thread_id) {
                events.push(entry);
            }
        }

        Ok(events)
    }

    pub fn pending_requests(&self) -> Result<Vec<Value>> {
        let mut requests = self
            .pending_requests
            .lock()
            .map_err(|_| {
                AppError::CodexCommand("failed to lock app-server pending requests".to_owned())
            })?
            .values()
            .filter(|request| is_approval_request(request))
            .map(redact_json)
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| {
            request_key(left.get("id").unwrap_or(&Value::Null))
                .cmp(&request_key(right.get("id").unwrap_or(&Value::Null)))
        });
        Ok(requests)
    }

    pub fn respond(&self, request_id: u64, result: Value) -> Result<()> {
        self.respond_value(Value::from(request_id), result)
    }

    pub fn respond_value(&self, request_id: Value, result: Value) -> Result<()> {
        let key = request_key(&request_id);
        let pending_request = {
            let mut pending = self.pending_requests.lock().map_err(|_| {
                AppError::CodexCommand("failed to lock app-server pending requests".to_owned())
            })?;
            let request = pending.get(&key).cloned().ok_or_else(|| {
                AppError::CodexCommand(format!("app-server request id not pending: {key}"))
            })?;
            if !is_approval_request(&request) {
                return Err(AppError::CodexCommand(format!(
                    "app-server request id is not an approval: {key}"
                )));
            }
            pending.remove(&key).expect("pending request was checked")
        };

        let response = self.write_envelope(&json!({
            "id": request_id,
            "result": result,
        }));
        if let Err(err) = response {
            if let Ok(mut pending) = self.pending_requests.lock() {
                pending.insert(key, pending_request);
            }
            return Err(err);
        }
        Ok(())
    }

    fn call_raw(&self, method: &str, params: Value) -> Result<Value> {
        self.unwrapped_call(method, params)
    }

    fn unwrapped_call(&self, method: &str, params: Value) -> Result<Value> {
        let result = match self.call_rpc(method, params.clone()) {
            Ok(result) => result,
            Err(err) if method != "initialize" && err.to_string().contains("app-server exited") => {
                self.reset_session();
                self.ensure_initialized()?;
                self.call_rpc(method, params)?
            }
            Err(err) => return Err(err),
        };

        if let Some(method) = result.get("method").and_then(Value::as_str) {
            return Err(AppError::CodexCommand(format!(
                "unexpected notification for {method}"
            )));
        }

        if let Some(error) = result.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown app-server error");
            return Err(AppError::CodexCommand(format!(
                "app-server error: {message}"
            )));
        }

        result
            .get("result")
            .cloned()
            .ok_or_else(|| AppError::CodexCommand("app-server returned no result".to_owned()))
    }

    fn ensure_session(&self) -> Result<()> {
        let mut session_guard = self.session.lock().map_err(|_| {
            AppError::CodexCommand("failed to lock app-server session mutex".to_owned())
        })?;

        if let Some(session) = session_guard.as_mut() {
            if session
                .child
                .try_wait()
                .map_err(|source: std::io::Error| AppError::CodexCommand(source.to_string()))?
                .is_none()
            {
                return Ok(());
            }
            self.initialized.store(false, Ordering::Release);
        }

        let mut command = Command::new(&self.binary);
        command
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|source| AppError::CodexCommand(source.to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::CodexCommand("app-server stdout pipe missing".to_owned()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::CodexCommand("app-server stdin pipe missing".to_owned()))?;
        let stderr = child.stderr.take();

        let waiters = Arc::clone(&self.waiters);
        let pending_requests = Arc::clone(&self.pending_requests);
        let event_path = self.event_log_path();
        let event_writer_guard = Arc::new(Mutex::new(()));
        fs::create_dir_all(&self.state_dir).map_err(|source| AppError::WriteFile {
            path: self.state_dir.clone(),
            source,
        })?;

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for maybe_line in reader.lines() {
                let line = match maybe_line {
                    Ok(line) => line,
                    Err(_) => break,
                };
                let Ok(envelope) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if envelope.get("id").is_some() && envelope.get("method").is_some() {
                    if let Ok(mut pending) = pending_requests.lock() {
                        if let Some(id) = envelope.get("id") {
                            pending.insert(request_key(id), envelope.clone());
                        }
                    }
                    append_event(&event_writer_guard, &event_path, &envelope);
                    continue;
                }
                if let Some(id) = extract_u64_id(&envelope) {
                    if let Some(sender) = waiters.lock().ok().and_then(|mut lock| lock.remove(&id))
                    {
                        let _ = sender.send(Ok(envelope));
                        continue;
                    }
                }
                if envelope.get("method").is_some() {
                    append_event(&event_writer_guard, &event_path, &envelope);
                }
            }
            if let Ok(mut waiters) = waiters.lock() {
                for (_, sender) in waiters.drain() {
                    let _ =
                        sender.send(Err(AppError::CodexCommand("app-server exited".to_owned())));
                }
            }
        });

        if let Some(stderr) = stderr {
            thread::spawn(move || {
                let mut stderr = BufReader::new(stderr);
                let mut buf = String::new();
                loop {
                    match stderr.read_line(&mut buf) {
                        Ok(0) => break,
                        Ok(_) => {
                            buf.clear();
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        *session_guard = Some(AppServerSession {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
        });

        Ok(())
    }

    fn write_envelope(&self, envelope: &Value) -> Result<()> {
        self.ensure_session()?;

        let request = envelope.to_string() + "\n";

        let session = self.session.lock().map_err(|_| {
            AppError::CodexCommand("failed to lock app-server session for write".to_owned())
        })?;
        let Some(session) = session.as_ref() else {
            return Err(AppError::CodexCommand(
                "app-server session missing".to_owned(),
            ));
        };
        let mut stdin = session
            .stdin
            .lock()
            .map_err(|_| AppError::CodexCommand("failed to lock app-server stdin".to_owned()))?;
        stdin
            .write_all(request.as_bytes())
            .and_then(|_| stdin.flush())
            .map_err(|source: std::io::Error| AppError::CodexCommand(source.to_string()))
            .map(|_| ())
    }

    fn send_notification(&self, method: &str, params: Value) -> Result<()> {
        self.write_envelope(&json!({
            "method": method,
            "params": params,
        }))
    }

    fn call_rpc(&self, method: &str, params: Value) -> Result<Value> {
        self.ensure_session()?;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = json!({
            "id": id,
            "method": method,
            "params": params,
        });

        let (sender, receiver) = mpsc::channel::<RpcResult>();
        {
            let mut waiters = self.waiters.lock().map_err(|_| {
                AppError::CodexCommand("failed to lock app-server waiters".to_owned())
            })?;
            waiters.insert(id, sender);
        }

        if let Err(err) = self.write_envelope(&request) {
            let mut waiters = self.waiters.lock().map_err(|_| {
                AppError::CodexCommand("failed to lock app-server waiters".to_owned())
            })?;
            waiters.remove(&id);
            return Err(err);
        }

        let response = receiver
            .recv_timeout(Duration::from_secs(120))
            .map_err(|_| AppError::CodexCommand("app-server response timed out".to_owned()));

        if response.is_err() {
            let mut waiters = self.waiters.lock().map_err(|_| {
                AppError::CodexCommand("failed to lock app-server waiters".to_owned())
            })?;
            waiters.remove(&id);
        }

        response?
    }

    fn event_log_path(&self) -> PathBuf {
        self.state_dir.join("appserver-events.jsonl")
    }

    fn reset_session(&self) {
        if let Ok(mut session) = self.session.lock() {
            *session = None;
        }
        self.initialized.store(false, Ordering::Release);
    }
}

fn append_event(writer_lock: &Arc<Mutex<()>>, path: &Path, envelope: &Value) {
    let payload = redact_json(envelope);
    let event = json!({
        "ts": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0),
        "method": envelope.get("method").cloned().unwrap_or_else(|| json!(null)),
        "thread_id": event_thread_id(envelope).unwrap_or("unknown"),
        "payload": payload,
    });

    if let Ok(_guard) = writer_lock.lock() {
        if let Ok(mut out) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = out.write_all(event.to_string().as_bytes());
            let _ = out.write_all(b"\n");
        }
    }
}

fn redact_json(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(redact_for_log(text)),
        Value::Array(items) => Value::Array(items.iter().map(redact_json).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    if key.eq_ignore_ascii_case("id") || key.eq_ignore_ascii_case("method") {
                        (key.clone(), value.clone())
                    } else if should_redact_key(key) {
                        match value {
                            Value::String(_) => {
                                (key.clone(), Value::String("[REDACTED]".to_owned()))
                            }
                            _ => (key.clone(), redact_json(value)),
                        }
                    } else {
                        (key.clone(), redact_json(value))
                    }
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn should_redact_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("token")
        || key.contains("secret")
        || key.contains("api_key")
        || key.contains("apikey")
        || key.contains("credential")
        || key == "authorization"
}

fn is_approval_request(value: &Value) -> bool {
    value
        .get("method")
        .and_then(Value::as_str)
        .is_some_and(|method| method.to_ascii_lowercase().contains("approval"))
}

fn event_thread_id(envelope: &Value) -> Option<&str> {
    envelope
        .get("params")
        .and_then(|params| params.get("threadId").and_then(Value::as_str))
        .or_else(|| {
            envelope
                .get("params")
                .and_then(|params| params.get("thread_id").and_then(Value::as_str))
        })
        .or_else(|| payload_thread_id(envelope))
}

fn payload_thread_id(envelope: &Value) -> Option<&str> {
    envelope
        .get("threadId")
        .and_then(Value::as_str)
        .or_else(|| envelope.get("thread_id").and_then(Value::as_str))
}

fn extract_u64_id(envelope: &Value) -> Option<u64> {
    envelope.get("id").and_then(Value::as_u64).or_else(|| {
        envelope
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| id.parse().ok())
    })
}

fn request_key(id: &Value) -> String {
    id.as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| id.to_string())
}
