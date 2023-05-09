use anyhow::{anyhow, Context as _};
use awc::Client;
use clap::Parser;

use config_api::CommonArgs;

#[derive(Parser)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let url = format!(
        "http://127.0.0.1:{}/health",
        args.common.listen_address.port()
    );

    let client = Client::default();
    let mut resp = client
        .get(url.as_str())
        .send()
        .await
        .map_err(|err| anyhow!(err.to_string()))
        .context("request error")?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let body = resp.body().await?;
        let utf8_body = String::from_utf8_lossy(&body).into_owned();
        Err(anyhow!(utf8_body))
    }
}
