use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use axum::http::StatusCode;
use clap::Args;
use futures_util::TryFutureExt;
use mongodb::bson::{self, doc, Bson, Document};
use mongodb::options::{ClientOptions, CountOptions};
use mongodb::Client;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, info_span, instrument, warn, Instrument};

use crate::channel::{roundtrip_channel, RoundtripSender};

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

pub(crate) type HealthChannel = RoundtripSender<(), bool>;

#[derive(Debug)]
pub(crate) struct GetConfigRequest {
    pub(crate) collection: String,
    pub(crate) id: String,
}

#[derive(Debug)]
pub(crate) enum GetConfigResponse {
    Document(Document),
    NotFound(String),
    Error,
}

pub(crate) type GetConfigChannel = RoundtripSender<GetConfigRequest, GetConfigResponse>;

pub(crate) struct PatchConfigRequest {
    pub(crate) collection: String,
    pub(crate) id: String,
    pub(crate) changes: HashMap<String, Bson>,
}

pub(crate) type PatchConfigChannel = RoundtripSender<PatchConfigRequest, StatusCode>;

#[derive(Clone)]
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

    pub(crate) fn handle_health(&self) -> (HealthChannel, JoinHandle<()>) {
        let (tx, mut rx) = roundtrip_channel(1);
        let command = doc! { "ping": 1 };
        let cloned_self = self.clone();

        let task = tokio::spawn(
            async move {
                info!(status = "started");
                while let Some((_, response_tx)) = rx.recv().await {
                    debug!(msg = "request received");
                    let outcome = cloned_self
                        .0
                        .run_command(command.clone(), None)
                        .await
                        .is_ok();
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

    pub(crate) fn handle_get_config(&self) -> (GetConfigChannel, JoinHandle<()>) {
        let (tx, mut rx) = roundtrip_channel::<GetConfigRequest, GetConfigResponse>(5);
        let cloned_self = self.clone();

        let task = tokio::spawn(
            async move {
                info!(status = "started");
                while let Some((request, response_tx)) = rx.recv().await {
                    debug!(msg = "request received", ?request);
                    let collection = cloned_self.0.collection::<Document>(&request.collection);
                    let mut document_id = request.id;
                    let filter = doc! { "_id": &document_id };
                    let found = collection
                        .find_one(filter, None)
                        .and_then(|first_found| async {
                            if let Some(Bson::ObjectId(links_id)) =
                                first_found.as_ref().and_then(|doc| doc.get("_links"))
                            {
                                document_id = links_id.to_string();
                                let filter = doc! { "_id": links_id };
                                collection.find_one(filter, None).await
                            } else {
                                Ok(first_found)
                            }
                        })
                        .await;
                    let response = match found {
                        Ok(Some(doc)) => GetConfigResponse::Document(doc),
                        Ok(None) => GetConfigResponse::NotFound(format!(
                            "Document with id `{}` not found in `{}` collection",
                            document_id, request.collection
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

    pub(crate) fn handle_patch_config(&self) -> (PatchConfigChannel, JoinHandle<()>) {
        let (tx, mut rx) = roundtrip_channel::<PatchConfigRequest, StatusCode>(10);
        let cloned_self = self.clone();

        let task = tokio::spawn(
            async move {
                info!(status = "started");

                while let Some((request, reply_tx)) = rx.recv().await {
                    let send_reply = |reply: StatusCode| {
                        if reply_tx.send(reply).is_err() {
                            error!(kind = "reply channel sending");
                        }
                    };
                    let collection = cloned_self.0.collection::<Document>(&request.collection);
                    let requested_changes_keys = request.changes.keys().collect::<Vec<_>>();
                    let auth_document_filter = doc! {
                        "_id": "_authorization",
                        "patchAllowedFields": {
                            "$all": &requested_changes_keys,
                        },
                    };
                    let auth_document_options = CountOptions::builder().limit(1).build();
                    match collection
                        .count_documents(auth_document_filter, auth_document_options)
                        .await
                    {
                        Ok(count) if count == 0 => {
                            warn!(
                                msg = "missing authorization",
                                request.collection,
                                ?requested_changes_keys
                            );
                            send_reply(StatusCode::UNAUTHORIZED);
                            continue;
                        }
                        Err(err) => {
                            error!(kind = "document count request", request.collection, %err);
                            continue;
                        }
                        Ok(_) => {}
                    }
                    let update_filter = doc! { "_id": request.id };
                    let update_document = match bson::to_document(&request.changes) {
                        Ok(doc) => doc,
                        Err(err) => {
                            error!(kind = "encoding changes document", request.collection, %err);
                            continue;
                        }
                    };
                    let update = doc! { "$set": update_document };
                    if let Err(err) = collection.update_one(update_filter, update, None).await {
                        error!(kind = "document updating", request.collection, %err);
                    } else {
                        send_reply(StatusCode::OK);
                    }
                }

                info!(status = "terminating");
            }
            .instrument(info_span!("mongodb_patch_config_handler")),
        );

        (tx, task)
    }
}
