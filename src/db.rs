use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use arcstr::ArcStr;
use clap::Args;
use mongodb::bson::{doc, Document};
use mongodb::options::ClientOptions;
use mongodb::Client;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, info_span, instrument, Instrument};

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

#[derive(Debug)]
pub(crate) struct GetConfigRequest {
    pub(crate) collection: ArcStr,
    pub(crate) id: ArcStr,
}

#[derive(Debug)]
pub(crate) enum GetConfigResponse {
    Document(Document),
    NotFound(String),
    Error,
}

pub(crate) type GetConfigMessage = (GetConfigRequest, oneshot::Sender<GetConfigResponse>);

pub(crate) struct Database(mongodb::Database);

impl Database {
    #[instrument(skip_all)]
    pub(crate) async fn create(config: &Config) -> anyhow::Result<Self> {
        let mut options = ClientOptions::parse(&config.mongodb_uri)
            .await
            .context("error parsing connection string URI")?;
        options.app_name = APP_NAME.to_string().into();
        options.server_selection_timeout = Duration::from_secs(2).into();
        let client = Client::with_options(options).context("error creating the client")?;
        let database = client.database(&config.mongodb_database);
        info!(status = "success");
        Ok(Self(database))
    }

    pub(crate) fn handle_health(
        self: Arc<Self>,
    ) -> (mpsc::Sender<oneshot::Sender<bool>>, JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<oneshot::Sender<bool>>(1);
        let command = doc! { "ping": 1 };

        let task = tokio::spawn(
            async move {
                info!(status = "started");
                while let Some(response_tx) = rx.recv().await {
                    debug!(msg = "request received");
                    let outcome = self.0.run_command(command.clone(), None).await.is_ok();
                    if response_tx.send(outcome).is_err() {
                        error!(kind = "outcome channel sending");
                    }
                }
                info!(status = "terminating");
            }
            .instrument(info_span!("mongodb_health_handler")),
        );

        (tx, task)
    }

    pub(crate) fn handle_get_config(
        self: Arc<Self>,
    ) -> (mpsc::Sender<GetConfigMessage>, JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<GetConfigMessage>(5);

        let task = tokio::spawn(
            async move {
                info!(status = "started");
                while let Some((request, response_tx)) = rx.recv().await {
                    debug!(msg = "request received", ?request);
                    let collection = self.0.collection::<Document>(&request.collection);
                    let filter = doc! { "_id": request.id.as_str() };
                    let response = match collection.find_one(filter, None).await {
                        Ok(Some(doc)) => GetConfigResponse::Document(doc),
                        Ok(None) => GetConfigResponse::NotFound(format!(
                            "Document with id `{}` not found in `{}` collection",
                            request.id, request.collection
                        )),
                        Err(err) => {
                            error!(during = "document finding", %err);
                            GetConfigResponse::Error
                        }
                    };
                    if response_tx.send(response).is_err() {
                        error!(kind = "outcome channel sending");
                    }
                }
                info!(status = "terminating");
            }
            .instrument(info_span!("mongodb_get_config_handler")),
        );

        (tx, task)
    }
}
