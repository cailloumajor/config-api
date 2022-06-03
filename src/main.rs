use anyhow::{Context, Ok};
use async_std::sync::{Arc, RwLock};

mod config;

use config::handler as config_handler;

#[derive(Clone)]
pub struct AppState {
    toml_value: Arc<RwLock<toml::Value>>,
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let mut app = tide::with_state(AppState {
        toml_value: Arc::new(RwLock::new(
            // TODO: implement real-world
            toml::from_str(include_str!("../tests/test.toml")).unwrap(),
        )),
    });
    app.at("/config/*path").get(config_handler);
    app.listen("0.0.0.0:8080").await.context("listen error")?;

    Ok(())
}
