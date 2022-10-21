use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context;
use async_std::channel::{self, Receiver};
use async_std::prelude::*;
use async_std::sync::{Arc, RwLock};
use async_std::task;
use clap::Parser;
use notify::{recommended_watcher, Event, RecursiveMode, Watcher};
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::low_level::signal_name;
use signal_hook_async_std::Signals;
use trillium::{Conn, Handler, KnownHeaderName, State};
use trillium_async_std::Stopper;
use trillium_caching_headers::EntityTag;
use trillium_router::Router;

mod config;
mod health;

use config::{handler as config_handler, load_config};
use health::handler as health_handler;

#[derive(Parser)]
struct Args {
    /// Address to listen on
    #[arg(env, long, default_value = "0.0.0.0:8080", action)]
    listen_address: SocketAddr,

    /// Path of the static configuration TOML file
    #[arg(env, long, action)]
    config_path: String,
}

#[derive(Clone)]
pub struct StaticConfig {
    data: serde_json::Value,
    etag: EntityTag,
}

type SafeStaticConfig = Arc<RwLock<Option<StaticConfig>>>;

#[derive(Clone)]
pub struct AppState {
    static_config: SafeStaticConfig,
}

async fn handle_signals(mut signals: Signals, stopper: Stopper) {
    while let Some(signal) = signals.next().await {
        let signame = signal_name(signal).unwrap_or("unknown");
        eprintln!(
            r#"from=handle_signal signal={} msg="received signal, exiting""#,
            signame
        );
        stopper.stop();
    }
    eprintln!("from=handle_signal status=exiting");
}

async fn handle_notify(
    mut rx: Receiver<notify::Result<Event>>,
    config_path: Arc<String>,
    static_config: SafeStaticConfig,
) {
    while let Some(result) = rx.next().await {
        match result {
            Ok(event) => {
                if event.kind.is_modify() {
                    eprintln!(r#"from=handle_notify msg="reloading configuration""#);
                    let mut config_write = static_config.write().await;
                    match load_config(&config_path).await {
                        Ok(new_config) => {
                            *config_write = Some(new_config);
                        }
                        Err(err) => {
                            *config_write = None;
                            eprintln!("from=handle_notify during=load_config err={err}");
                        }
                    }
                }
            }
            Err(err) => eprintln!("from=handle_notify during=match_result err={err}"),
        }
    }
    eprintln!("from=handle_notify status=exiting");
}

async fn remove_server_header(mut conn: Conn) -> Conn {
    conn.headers_mut().remove(KnownHeaderName::Server);
    conn
}

fn router() -> impl Handler {
    Router::new()
        .get("/config/*", config_handler)
        .get("/health", health_handler)
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let trillium_stopper = trillium_async_std::Stopper::new();
    let signals = Signals::new(TERM_SIGNALS).context("error registering termination signals")?;
    let signals_handle = signals.handle();
    let signal_task = task::spawn(handle_signals(signals, trillium_stopper.clone()));

    let static_config = Arc::new(RwLock::new(Some(
        load_config(&args.config_path)
            .await
            .context("error during initial load of static configuration")?,
    )));

    let (notify_tx, notify_rx) = channel::bounded(1);
    let watch_config_task = task::spawn(handle_notify(
        notify_rx,
        Arc::new(args.config_path.clone()),
        static_config.clone(),
    ));
    let cloned_notify_tx = notify_tx.clone();
    let mut last_sent = Instant::now();
    let mut watcher = recommended_watcher(move |result: notify::Result<Event>| {
        // Debounce sending to the channel
        if last_sent.elapsed() < Duration::from_millis(100) {
            return;
        }
        if let Err(err) = cloned_notify_tx.try_send(result) {
            eprintln!("from=watcher_event_handler err={err}")
        }
        last_sent = Instant::now();
    })
    .context("error creating static configuration file watcher")?;
    watcher
        .watch(Path::new(&args.config_path), RecursiveMode::NonRecursive)
        .context("error starting static configuration file watcher")?;

    eprintln!(r#"listen="{}" msg="start listening""#, args.listen_address);
    trillium_async_std::config()
        .with_host(&args.listen_address.ip().to_string())
        .with_port(args.listen_address.port())
        .without_signals()
        .with_stopper(trillium_stopper)
        .run_async((
            State::new(AppState { static_config }),
            remove_server_header,
            router(),
        ))
        .await;

    notify_tx.close();
    signals_handle.close();
    watch_config_task.await;
    signal_task.await;

    Ok(())
}
