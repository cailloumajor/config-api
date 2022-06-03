use anyhow::Context;
use async_std::prelude::*;
use async_std::sync::{Arc, RwLock};
use clap::Parser;

mod config;

use config::handler as config_handler;
use futures::future::{AbortHandle, Abortable, Aborted};
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::low_level::signal_name;
use signal_hook_async_std::Signals;

#[derive(Parser)]
#[clap(name = "Static configuration API")]
#[clap(about)]
struct Args {
    /// Address to listen on
    #[clap(long, default_value = "0.0.0.0:8080")]
    listen_address: String,
}

#[derive(Clone)]
pub struct AppState {
    toml_value: Arc<RwLock<toml::Value>>,
}

async fn handle_signals(mut signals: Signals, abort_handle: AbortHandle) {
    while let Some(signal) = signals.next().await {
        let signame = signal_name(signal).unwrap_or("unknown");
        eprintln!(r#"signal={} msg="received signal, exiting""#, signame);
        abort_handle.abort();
    }
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let signals = Signals::new(TERM_SIGNALS)?;
    let signals_handle = signals.handle();
    let signal_task = async_std::task::spawn(handle_signals(signals, abort_handle));

    let mut app = tide::with_state(AppState {
        toml_value: Arc::new(RwLock::new(
            // TODO: implement real-world
            toml::from_str("[main]\nwip_data = true").unwrap(),
        )),
    });
    app.at("/config/*path").get(config_handler);
    eprintln!(r#"listen="{}" msg="start listening""#, args.listen_address);

    let listen_future = Abortable::new(app.listen(args.listen_address), abort_registration);
    match listen_future.await {
        Err(Aborted) => Ok(()),
        Ok(listen_result) => listen_result.context("listen error"),
    }?;

    signals_handle.close();
    signal_task.await;

    Ok(())
}
