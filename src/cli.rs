use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Serve { config_path: PathBuf },
    Help,
}

pub fn parse_args<I, S>(args: I) -> Result<CliCommand, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();
    let command = args.next();

    match command.as_deref() {
        Some("-h") | Some("--help") | Some("help") => Ok(CliCommand::Help),
        None | Some("serve") => parse_serve(args),
        Some(other) => Err(format!("unknown command: {other}")),
    }
}

fn parse_serve<I>(mut args: I) -> Result<CliCommand, String>
where
    I: Iterator<Item = String>,
{
    let mut config_path = PathBuf::from("codex2gpt.json");

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliCommand::Help),
            "--config" => {
                let Some(path) = args.next() else {
                    return Err("--config requires a path".to_owned());
                };
                config_path = PathBuf::from(path);
            }
            other => return Err(format!("unknown serve option: {other}")),
        }
    }

    Ok(CliCommand::Serve { config_path })
}

pub fn usage() -> &'static str {
    "usage: codex2gpt [serve] [--config PATH]"
}
