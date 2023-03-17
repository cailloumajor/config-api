use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use clap_verbosity_flag::{InfoLevel, LogLevel, Verbosity};
use futures_util::StreamExt;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::low_level::signal_name;
use signal_hook_tokio::Signals;
use tracing::{info, info_span, instrument, Instrument};
use tracing_log::LogTracer;
use trillium_tokio::Stopper;

use config_api::CommonArgs;
use db::Database;

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

fn filter_from_verbosity<T>(verbosity: &Verbosity<T>) -> tracing::level_filters::LevelFilter
where
    T: LogLevel,
{
    use tracing_log::log::LevelFilter;
    match verbosity.log_level_filter() {
        LevelFilter::Off => tracing::level_filters::LevelFilter::OFF,
        LevelFilter::Error => tracing::level_filters::LevelFilter::ERROR,
        LevelFilter::Warn => tracing::level_filters::LevelFilter::WARN,
        LevelFilter::Info => tracing::level_filters::LevelFilter::INFO,
        LevelFilter::Debug => tracing::level_filters::LevelFilter::DEBUG,
        LevelFilter::Trace => tracing::level_filters::LevelFilter::TRACE,
    }
}

#[instrument(skip_all)]
async fn handle_signals(signals: Signals, stopper: Stopper) {
    let mut signals_stream = signals.map(|signal| signal_name(signal).unwrap_or("unknown"));
    info!(status = "started");
    while let Some(signal) = signals_stream.next().await {
        info!(msg = "received signal", reaction = "shutting down", signal);
        stopper.stop();
    }
    info!(status = "terminating");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_max_level(filter_from_verbosity(&args.verbose))
        .init();

    LogTracer::init_with_filter(args.verbose.log_level_filter())?;

    let api_stopper = trillium_tokio::Stopper::new();

    let signals = Signals::new(TERM_SIGNALS).context("error registering termination signals")?;
    let signals_handle = signals.handle();
    let signals_task = tokio::spawn(handle_signals(signals, api_stopper.clone()));

    let database = Database::create(&args.mongodb).await?;
    let database = Arc::new(database);
    let (database_health_tx, database_health_task) = database.clone().handle_health();
    let (get_config_tx, get_config_task) = database.handle_get_config();

    let api_handler = http_api::handler(database_health_tx, get_config_tx);
    async move {
        info!(addr = %args.common.listen_address, msg = "start listening");
        trillium_tokio::config()
            .with_host(&args.common.listen_address.ip().to_string())
            .with_port(args.common.listen_address.port())
            .without_signals()
            .with_stopper(api_stopper)
            .run_async(api_handler)
            .await;
        info!(status = "terminating");
    }
    .instrument(info_span!("http_server_task"))
    .await;

    signals_handle.close();

    tokio::try_join!(signals_task, database_health_task, get_config_task)
        .context("error joining signals handling task")?;

    Ok(())
}
