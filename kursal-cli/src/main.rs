use kursal_cli::CLIArgs;
use clap::Parser;

#[tokio::main]
pub async fn main() {
    let args = CLIArgs::parse();

    kursal_cli::run(args.config, args.validate, args.default_config).await;
}
