use std::fs;

use anyhow::Context;
use tokio::signal::ctrl_c;
use webhook_server::{Config, Server};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = fs::read("webhook.toml").context("Failed to open `webhook.toml`")?;
    let config: Config = toml::from_slice(&config).context("Failed to parse `webhook.toml`")?;

    let server = Server::new(config)
        .await
        .context("Failed to start the server")?;
    let ctrl_c = ctrl_c();

    log::info!("Starting the server");

    tokio::select! {
        result = server => {
            result?;
            log::info!("The server has exited");
        }
        result = ctrl_c => {
            result?;
            log::info!("Received SIGINT, exiting");
        }
    }

    Ok(())
}
