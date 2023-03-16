use std::time::Duration;

use anyhow::Context;
use clap::Args;
use mongodb::options::ClientOptions;
use mongodb::{Client, Database};
use tracing::{info, instrument};

const APP_NAME: &str = concat!(env!("CARGO_PKG_NAME"), " (", env!("CARGO_PKG_VERSION"), ")");

#[derive(Args)]
pub(crate) struct Config {
    /// URI of MongoDB server
    #[arg(env, long, default_value = "mongodb://mongodb")]
    mongodb_uri: String,

    /// MongoDB database
    #[arg(env, long)]
    mongodb_database: String,
}

#[instrument(skip_all)]
pub(crate) async fn get_database(config: &Config) -> anyhow::Result<Database> {
    let mut options = ClientOptions::parse(&config.mongodb_uri)
        .await
        .context("error parsing connection string URI")?;
    options.app_name = APP_NAME.to_string().into();
    options.server_selection_timeout = Duration::from_secs(2).into();
    let client = Client::with_options(options).context("error creating the client")?;
    let database = client.database(&config.mongodb_database);
    info!(status = "success");
    Ok(database)
}
