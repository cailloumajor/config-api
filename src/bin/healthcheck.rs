use anyhow::{anyhow, Context as _};
use clap::Parser;
use trillium_client::Client;
use trillium_tokio::ClientConfig;

use config_api::CommonArgs;

#[derive(Parser)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let url = format!(
        "http://127.0.0.1:{}/health",
        args.common.listen_address.port()
    );

    let client = Client::new(ClientConfig::default());
    let mut resp = client.get(url.as_str()).await?;

    let status = resp.status().context("missing status code")?;

    if !status.is_success() {
        let body = resp.response_body().read_string().await?;
        return Err(anyhow!(body));
    }

    Ok(())
}
