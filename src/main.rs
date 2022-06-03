use anyhow::Context;
use async_std::sync::{Arc, RwLock};
use clap::Parser;

mod config;

use config::handler as config_handler;

#[derive(Parser)]
#[clap(version, about)]
struct Args {
    /// Address to listen on
    #[clap(long, default_value = "0.0.0.0:8080")]
    listen_address: String,
}

#[derive(Clone)]
pub struct AppState {
    toml_value: Arc<RwLock<toml::Value>>,
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut app = tide::with_state(AppState {
        toml_value: Arc::new(RwLock::new(
            // TODO: implement real-world
            toml::from_str(include_str!("../tests/test.toml")).unwrap(),
        )),
    });
    app.at("/config/*path").get(config_handler);
    app.listen(args.listen_address)
        .await
        .context("listen error")?;

    Ok(())
}
