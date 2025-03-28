use anyhow::Context;
use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use futures_util::StreamExt;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::low_level::signal_name;
use signal_hook_tokio::Signals;
use tokio::net::TcpListener;
use tracing::{Instrument, error, info, info_span, instrument};
use tracing_log::LogTracer;

use config_api::CommonArgs;
use db::Database;

mod channel;
mod db;
mod http_api;

#[derive(Parser)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    #[command(flatten)]
    mongodb: db::Config,

    #[command(flatten)]
    verbose: Verbosity<InfoLevel>,
}

#[instrument(skip_all)]
async fn handle_signals(signals: Signals) {
    let mut signals_stream = signals.map(|signal| signal_name(signal).unwrap_or("unknown"));
    info!(status = "started");
    if let Some(signal) = signals_stream.next().await {
        info!(msg = "received signal", reaction = "shutting down", signal);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_max_level(args.verbose.tracing_level())
        .init();

    LogTracer::init_with_filter(args.verbose.log_level_filter())?;

    let database = Database::create(&args.mongodb).await?;
    let (health_channel, health_task) = database.clone().handle_health();
    let (get_collection_channel, get_collection_task) = database.handle_get_collection();
    let (get_document_channel, get_document_task) = database.handle_get_document();
    let (patch_config_channel, patch_config_task) = database.handle_patch_config();

    let signals = Signals::new(TERM_SIGNALS).context("error registering termination signals")?;
    let signals_handle = signals.handle();

    let app = http_api::app(http_api::AppState {
        health_channel,
        get_collection_channel,
        get_document_channel,
        patch_config_channel,
    });
    async move {
        let listener = match TcpListener::bind(&args.common.listen_address).await {
            Ok(listener) => {
                info!(addr = %args.common.listen_address, msg = "listening");
                listener
            }
            Err(err) => {
                error!(kind="TCP listen", %err);
                return;
            }
        };
        if let Err(err) = axum::serve(listener, app.into_make_service())
            .with_graceful_shutdown(handle_signals(signals))
            .await
        {
            error!(kind = "HTTP server", %err);
        }
        info!(status = "terminating");
    }
    .instrument(info_span!("http_server_task"))
    .await;

    signals_handle.close();

    tokio::try_join!(
        health_task,
        get_collection_task,
        get_document_task,
        patch_config_task
    )
    .context("error joining task(s)")?;

    Ok(())
}
