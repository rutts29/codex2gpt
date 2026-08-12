use codex2gpt::cli::{CliCommand, parse_args, usage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    match parse_args(std::env::args())? {
        CliCommand::Serve { config_path } => codex2gpt::server::serve(&config_path).await?,
        CliCommand::Help => println!("{}", usage()),
    }

    Ok(())
}
