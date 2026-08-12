use serde_json::{Value, json};

use crate::config::{AppConfig, ToolSurface};

pub(crate) const RESULT_BUNDLE_WIDGET_URI: &str = "ui://codex2gpt/result-bundle-v2.html";
pub(crate) const APPROVALS_WIDGET_URI: &str = "ui://codex2gpt/approvals-v2.html";
pub(crate) const THREADS_WIDGET_URI: &str = "ui://codex2gpt/threads-v2.html";
pub(crate) const WORKTREES_WIDGET_URI: &str = "ui://codex2gpt/worktrees-v2.html";
const WIDGET_MIME_TYPE: &str = "text/html;profile=mcp-app";

pub fn resource_descriptors(config: &AppConfig) -> Vec<Value> {
    if config.tool_surface != ToolSurface::Full {
        return Vec::new();
    }
    vec![
        widget_resource_descriptor(RESULT_BUNDLE_WIDGET_URI, "Codex Result Bundle"),
        widget_resource_descriptor(APPROVALS_WIDGET_URI, "Codex Approvals"),
        widget_resource_descriptor(THREADS_WIDGET_URI, "Codex Threads"),
        widget_resource_descriptor(WORKTREES_WIDGET_URI, "Codex Worktrees"),
    ]
}

fn widget_resource_descriptor(uri: &str, name: &str) -> Value {
    json!({
        "uri": uri,
        "name": name,
        "title": name,
        "mimeType": WIDGET_MIME_TYPE,
    })
}

pub fn read_resource(config: &AppConfig, uri: &str) -> Result<Value, &'static str> {
    if config.tool_surface != ToolSurface::Full {
        return Err("unknown widget resource");
    }
    let Some((name, html, description)) = widget_resource(uri) else {
        return Err("unknown widget resource");
    };
    let mut meta = json!({
        "ui": {
            "csp": {
                "connectDomains": [],
                "resourceDomains": []
            },
            "prefersBorder": true
        },
        "openai/widgetDescription": description,
        "openai/widgetPrefersBorder": true,
        "openai/widgetCSP": {
            "connect_domains": [],
            "resource_domains": []
        }
    });
    if let Some(domain) = &config.widget_domain {
        meta["ui"]["domain"] = json!(domain);
        meta["openai/widgetDomain"] = json!(domain);
    }

    Ok(json!({
        "contents": [{
            "uri": uri,
            "name": name,
            "mimeType": WIDGET_MIME_TYPE,
            "text": html,
            "_meta": meta
        }]
    }))
}

fn widget_resource(uri: &str) -> Option<(&'static str, String, &'static str)> {
    Some(match uri {
        RESULT_BUNDLE_WIDGET_URI => (
            "Codex Result Bundle",
            result_bundle_widget_html(),
            "Shows Codex result status, changed files, commands, tests, token usage, and recent events.",
        ),
        APPROVALS_WIDGET_URI => (
            "Codex Approvals",
            approvals_widget_html(),
            "Shows pending Codex approval requests and the fields needed to approve or deny them.",
        ),
        THREADS_WIDGET_URI => (
            "Codex Threads",
            threads_widget_html(),
            "Shows workspace-filtered Codex threads returned by the local bridge.",
        ),
        WORKTREES_WIDGET_URI => (
            "Codex Worktrees",
            worktrees_widget_html(),
            "Shows managed Git worktrees available to the local bridge.",
        ),
        _ => return None,
    })
}

const WIDGET_COMPONENT_CSS: &str = r#"
    * { box-sizing: border-box; }
    body { margin: 0; font: 13px/1.5 -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", system-ui, sans-serif; -webkit-font-smoothing: antialiased; color: var(--fg); background-color: var(--bg); background-image: var(--aura); background-repeat: no-repeat; }
    main { padding: 14px; display: grid; gap: 12px; }
    section, article { background: var(--glass-bg); -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-sat)); backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-sat)); border: 1px solid var(--glass-border); border-radius: var(--radius); padding: 12px; box-shadow: var(--glass-rim), var(--shadow); }
    h1, h2 { margin: 0 0 8px; line-height: 1.25; }
    h1 { font-size: 16px; font-weight: 600; letter-spacing: -0.01em; }
    h2 { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: .04em; color: var(--muted); }
    code { overflow-wrap: anywhere; }
    .grid { display: grid; gap: 8px; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); }
    .metric { background: var(--glass-bg-2); -webkit-backdrop-filter: blur(8px) saturate(var(--glass-sat)); backdrop-filter: blur(8px) saturate(var(--glass-sat)); border: 1px solid var(--glass-border); border-radius: var(--radius-sm); padding: 8px; box-shadow: var(--glass-rim); }
    .label { color: var(--muted); font-size: 10.5px; font-weight: 600; text-transform: uppercase; letter-spacing: .04em; }
    .value { margin-top: 4px; overflow-wrap: anywhere; font-weight: 600; }
    ul { margin: 0; padding-left: 18px; }
    li { margin: 2px 0; }
    pre { white-space: pre-wrap; overflow-wrap: anywhere; margin: 0; }
    .empty { color: var(--muted); }
    .status { display: inline-block; padding: 2px 8px; border-radius: 999px; font-size: 10.5px; font-weight: 600; text-transform: uppercase; letter-spacing: .03em; }
    .status-completed { background: var(--ok-soft); color: var(--ok); }
    .status-failed, .status-error, .status-canceled, .status-cancelled { background: var(--bad-soft); color: var(--bad); }
    .status-running { background: var(--warn-soft); color: var(--warn); }
    .status-unknown { background: var(--fill); color: var(--muted); }
    .count { display: inline-block; min-width: 16px; padding: 0 6px; border-radius: 999px; background: var(--fill); color: var(--muted); font-size: 10.5px; font-weight: 600; vertical-align: middle; }
    code.primary { display: block; font-weight: 600; color: var(--fg); word-break: break-all; }
    .head-row { display: flex; align-items: center; gap: 8px; }
    .mono { font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace; font-size: 11.5px; color: var(--muted); }
    .sub { color: var(--muted); font-size: 11.5px; margin-top: 2px; word-break: break-all; }
    .cmd { margin: 0 0 8px; }
    .cmd code { display: block; padding: 8px 10px; background: var(--fill); border-radius: var(--radius-sm); white-space: pre-wrap; overflow-wrap: anywhere; font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace; font-size: 12.5px; }
    .kv { display: grid; grid-template-columns: auto 1fr; gap: 2px 10px; margin: 0 0 8px; font-size: 12.5px; }
    .kv dt { color: var(--muted); }
    .kv dd { margin: 0; word-break: break-all; }
    .raw { font-size: 12px; }
    .raw summary { cursor: pointer; color: var(--muted); }
    .raw pre { margin-top: 6px; font-size: 11.5px; }
"#;

fn widget_head() -> String {
    let mut head = String::from(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="light dark">
  <style>"#,
    );
    head.push_str(crate::ui::THEME_TOKENS);
    head.push_str(WIDGET_COMPONENT_CSS);
    head.push_str("\n  </style>\n</head>\n");
    head
}

const WIDGET_SCRIPT_PRELUDE: &str = r#"  <script>
    const esc = (value) => String(value ?? "").replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#39;" }[ch]));
    const asArray = (value) => Array.isArray(value) ? value : [];
    const toolOutput = () => window.openai?.toolOutput || {};
    const awaiting = () => !window.openai || window.openai.toolOutput == null;
    const waitingView = (title) => `<h1>${esc(title)}</h1><article class="empty">Waiting for results…</article>`;
    function ready(render) {
      const update = () => { render(); window.openai?.notifyIntrinsicHeight?.(); };
      window.addEventListener("openai:set_globals", update, { passive: true });
      update();
    }
"#;

fn widget_document(script: &str) -> String {
    let mut html = widget_head();
    html.push_str("  <body>\n    <main id=\"app\"></main>\n");
    html.push_str(WIDGET_SCRIPT_PRELUDE);
    html.push_str(script);
    html.push_str("  </script>\n  </body>\n</html>");
    html
}

fn result_bundle_widget_html() -> String {
    widget_document(
        r##"
    function statusClass(status) {
      const value = String(status || "").toLowerCase();
      if (["completed", "complete", "succeeded", "success"].includes(value)) return "status-completed";
      if (["failed", "error", "canceled", "cancelled", "stopped"].includes(value)) return "status-failed";
      if (["running", "active", "streaming", "in_progress"].includes(value)) return "status-running";
      return "status-unknown";
    }
    function num(value) {
      const parsed = Number(value);
      return Number.isFinite(parsed) ? parsed.toLocaleString() : esc(value);
    }
    function counted(title, items, inner) {
      const count = asArray(items).length;
      return `<section><h2>${esc(title)}${count ? ` <span class="count">${count}</span>` : ""}</h2>${inner}</section>`;
    }
    function list(items) {
      return asArray(items).length ? `<ul>${asArray(items).map((item) => `<li>${esc(item)}</li>`).join("")}</ul>` : `<p class="empty">None recorded.</p>`;
    }
    function tokenTiles(usage) {
      const entries = Object.entries(usage || {}).filter(([, value]) => typeof value === "number" || typeof value === "string");
      if (!entries.length) return `<p class="empty">Not reported.</p>`;
      return `<div class="grid">${entries.map(([key, value]) => `<div class="metric"><div class="label">${esc(String(key).replace(/_/g, " "))}</div><div class="value">${typeof value === "number" ? num(value) : esc(value)}</div></div>`).join("")}</div>`;
    }
    function render() {
      const app = document.getElementById("app");
      if (awaiting()) { app.innerHTML = waitingView("Codex Result Bundle"); return; }
      const data = toolOutput();
      const status = data.status || "unknown";
      app.innerHTML = `
        <section>
          <h1>Codex Result Bundle</h1>
          <div class="grid">
            <div class="metric"><div class="label">Status</div><div class="value"><span class="status ${statusClass(status)}">${esc(status)}</span></div></div>
            <div class="metric"><div class="label">Thread</div><div class="value">${esc(data.thread_id || "unknown")}</div></div>
            <div class="metric"><div class="label">Branch</div><div class="value">${esc(data.branch || "unknown")}</div></div>
            <div class="metric"><div class="label">Events</div><div class="value">${num(asArray(data.events).length)}</div></div>
          </div>
        </section>
        <section><h2>Final Message</h2><pre>${esc(data.final_message || "")}</pre></section>
        ${counted("Diff Summary", data.diff_summary, list(data.diff_summary))}
        ${counted("Changed Files", data.changed_files, list(data.changed_files))}
        ${counted("Commands", data.commands_run, list(data.commands_run))}
        ${counted("Tests", data.tests_run, list(data.tests_run))}
        <section><h2>Token Usage</h2>${tokenTiles(data.token_usage)}</section>
      `;
    }
    ready(render);
"##,
    )
}

fn approvals_widget_html() -> String {
    widget_document(
        r##"
    const APPROVAL_LABELS = { execApproval: "Command approval", exec_command_approval: "Command approval", patchApproval: "Patch approval", apply_patch_approval: "Patch approval" };
    function requestTitle(request) {
      const raw = ["method", "type", "title", "name"].map((key) => request[key]).find((value) => value) || "approval";
      return APPROVAL_LABELS[raw] || raw;
    }
    function commandOf(request) {
      const cmd = (request.params && request.params.command) || request.command;
      if (Array.isArray(cmd)) return cmd.map(String).join(" ");
      if (typeof cmd === "string") return cmd;
      return "";
    }
    function fieldValue(request, key) {
      const value = (request.params && request.params[key]) || request[key];
      return value == null ? "" : String(value);
    }
    function approvalCard(request) {
      const command = commandOf(request);
      const thread = fieldValue(request, "threadId");
      const extra = JSON.stringify(request.params || {}, null, 2);
      let html = `<article><h2>${esc(requestTitle(request))}</h2>`;
      if (command) html += `<div class="cmd"><code>${esc(command)}</code></div>`;
      html += `<dl class="kv"><dt>Request</dt><dd><code>${esc(request.id)}</code></dd>`;
      if (thread) html += `<dt>Thread</dt><dd><code>${esc(thread)}</code></dd>`;
      html += `</dl>`;
      if (command) html += `<details class="raw"><summary>All details</summary><pre>${esc(extra)}</pre></details>`;
      else html += `<pre>${esc(extra)}</pre>`;
      html += `</article>`;
      return html;
    }
    function render() {
      const app = document.getElementById("app");
      if (awaiting()) { app.innerHTML = waitingView("Pending Codex Approvals"); return; }
      const pending = asArray(toolOutput().pending);
      app.innerHTML = `<h1>Pending Codex Approvals ${pending.length ? `<span class="count">${pending.length}</span>` : ""}</h1>` + (pending.length ? pending.map(approvalCard).join("") : `<article class="empty">No pending approval requests.</article>`);
    }
    ready(render);
"##,
    )
}

fn threads_widget_html() -> String {
    widget_document(
        r##"
    function render() {
      const app = document.getElementById("app");
      if (awaiting()) { app.innerHTML = waitingView("Codex Threads"); return; }
      const threads = asArray(toolOutput().threads);
      app.innerHTML = `<h1>Codex Threads ${threads.length ? `<span class="count">${threads.length}</span>` : ""}</h1>` + (threads.length ? threads.map((thread) => `
        <article><code class="primary">${esc(thread.cwd || thread.id || "")}</code>${thread.id ? `<div class="sub">${esc(thread.id)}</div>` : ""}</article>
      `).join("") : `<article class="empty">No threads returned for this workspace.</article>`);
    }
    ready(render);
"##,
    )
}

fn worktrees_widget_html() -> String {
    widget_document(
        r##"
    function render() {
      const app = document.getElementById("app");
      if (awaiting()) { app.innerHTML = waitingView("Managed Worktrees"); return; }
      const worktrees = asArray(toolOutput().worktrees);
      app.innerHTML = `<h1>Managed Worktrees ${worktrees.length ? `<span class="count">${worktrees.length}</span>` : ""}</h1>` + (worktrees.length ? worktrees.map((tree) => {
        const commit = tree.commit ? String(tree.commit).slice(0, 7) : "";
        return `
        <article>
          <div class="head-row"><strong>${esc(tree.branch || "detached")}</strong>${commit ? `<span class="mono">${esc(commit)}</span>` : ""}</div>
          <code>${esc(tree.path || "")}</code>
        </article>`;
      }).join("") : `<article class="empty">No managed worktrees returned.</article>`);
    }
    ready(render);
"##,
    )
}
