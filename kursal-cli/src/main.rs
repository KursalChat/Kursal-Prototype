use clap::Parser;
use kursal_cli::{config::RelayConfig, swarm::spawn_relay_swarm};
use kursal_core::logging::init_logging;
use std::path::PathBuf;

const DEFAULT_RELAY_TOML: &str = include_str!("../relay.example.toml");

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Config path for the relay (toml)
    #[arg(short, long, default_value = "relay.toml")]
    config: PathBuf,

    /// Returns success if the config file is valid
    #[arg(long)]
    validate: bool,

    /// Initiates a new toml config if none exists at the config path
    #[arg(long)]
    default_config: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.default_config && !std::fs::exists(&args.config).expect("Could not access the file") {
        std::fs::create_dir_all(
            args.config
                .parent()
                .expect("Could not get the parent directory"),
        )
        .expect("Could not create parent directory");
        std::fs::write(&args.config, DEFAULT_RELAY_TOML).expect("Could not write file");
    }

    let config = RelayConfig::load(&args.config).expect("Could not parse relay config");
    init_logging(&config.log_level, config.log_file.as_deref())
        .expect("Could not initiate logging");

    if args.validate {
        println!("Valid config file!");
        return;
    }

    let keypair_path = args
        .config
        .parent()
        .expect("config has no parent dir")
        .join("relay_identity.key");

    spawn_relay_swarm(&config, &keypair_path)
        .await
        .expect("Could not spawn swarm");
}
