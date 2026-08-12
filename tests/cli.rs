use std::path::PathBuf;

use codex2gpt::cli::{CliCommand, parse_args};

#[test]
fn parse_args_defaults_to_serve_with_default_config() {
    let command = parse_args(["codex2gpt"]).unwrap();

    assert_eq!(
        command,
        CliCommand::Serve {
            config_path: PathBuf::from("codex2gpt.json")
        }
    );
}

#[test]
fn parse_args_accepts_serve_config_path() {
    let command = parse_args(["codex2gpt", "serve", "--config", "/tmp/bridge.json"]).unwrap();

    assert_eq!(
        command,
        CliCommand::Serve {
            config_path: PathBuf::from("/tmp/bridge.json")
        }
    );
}

#[test]
fn parse_args_rejects_missing_config_value() {
    let err = parse_args(["codex2gpt", "serve", "--config"]).unwrap_err();

    assert!(err.contains("--config requires a path"));
}

#[test]
fn parse_args_accepts_help_flags() {
    assert_eq!(parse_args(["codex2gpt", "--help"]), Ok(CliCommand::Help));
    assert_eq!(
        parse_args(["codex2gpt", "serve", "--help"]),
        Ok(CliCommand::Help)
    );
}
