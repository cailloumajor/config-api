use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context;
use async_std::channel::{self, Receiver};
use async_std::prelude::*;
use async_std::sync::{Arc, RwLock};
use async_std::task;
use clap::Parser;
use futures::future::{AbortHandle, Abortable, Aborted};
use notify::{recommended_watcher, Event, RecursiveMode, Watcher};
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::low_level::signal_name;
use signal_hook_async_std::Signals;

mod config;
mod health;

use config::{handler as config_handler, load_config};
use health::handler as health_handler;

type StaticConfig = Arc<RwLock<Option<toml::Value>>>;

#[derive(Parser)]
#[clap(name = "Static configuration API")]
#[clap(about)]
struct Args {
    /// Address to listen on
    #[clap(env, long, default_value = "0.0.0.0:8080")]
    listen_address: String,

    /// Path of the static configuration TOML file
    #[clap(env, long)]
    config_path: String,
}

#[derive(Clone)]
pub struct AppState {
    static_config: StaticConfig,
}

async fn handle_signals(mut signals: Signals, abort_handle: AbortHandle) {
    while let Some(signal) = signals.next().await {
        let signame = signal_name(signal).unwrap_or("unknown");
        eprintln!(
            r#"from=handle_signal signal={} msg="received signal, exiting""#,
            signame
        );
        abort_handle.abort();
    }
    eprintln!("from=handle_signal status=exiting");
}

async fn handle_notify(
    mut rx: Receiver<notify::Result<Event>>,
    config_path: Arc<String>,
    static_config: StaticConfig,
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

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let signals = Signals::new(TERM_SIGNALS).context("error registering termination signals")?;
    let signals_handle = signals.handle();
    let signal_task = task::spawn(handle_signals(signals, abort_handle));

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

    let mut app = tide::with_state(AppState { static_config });
    app.at("/config/*path").get(config_handler);
    app.at("/health").get(health_handler);
    eprintln!(r#"listen="{}" msg="start listening""#, args.listen_address);

    let listen_future = Abortable::new(app.listen(args.listen_address), abort_registration);
    match listen_future.await {
        Err(Aborted) => Ok(()),
        Ok(listen_result) => listen_result.context("listen error"),
    }?;

    notify_tx.close();
    signals_handle.close();
    watch_config_task.await;
    signal_task.await;

    Ok(())
}
