use std::net::SocketAddr;

use clap::Args;

pub mod rfc7807;

#[derive(Args)]
pub struct CommonArgs {
    /// Address to listen on
    #[arg(env, long, default_value = "0.0.0.0:8080")]
    pub listen_address: SocketAddr,
}
