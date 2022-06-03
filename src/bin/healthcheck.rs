use anyhow::anyhow;
use clap::Parser;

#[derive(Parser)]
#[clap(name = "healthcheck")]
#[clap(about = "Static config API health checking")]
struct Args {
    /// Network port to use
    port: u16,
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let status = surf::get(format!("http://127.0.0.1:{}/health", args.port))
        .await
        .map_err(|e| e.into_inner())?
        .status();
    status
        .is_success()
        .then(|| ())
        .ok_or_else(|| anyhow!("unexpected status code: {}", status))
}
