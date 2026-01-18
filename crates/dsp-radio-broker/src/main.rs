//! DSPradio Broker Daemon
//!
//! A resource broker for managing SDR devices across a network.
//! Provides REST API and WebSocket streaming on port 8420.

use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod api;
mod broker;
mod device_registry;
mod lease;

use dsp_radio_protocol::DEFAULT_BROKER_PORT;

/// DSPradio SDR Resource Broker
#[derive(Parser, Debug)]
#[command(name = "dsp-radio-broker")]
#[command(about = "SDR resource broker daemon for DSPradio", long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = DEFAULT_BROKER_PORT, env = "DSPRADIO_PORT")]
    port: u16,

    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0", env = "DSPRADIO_BIND")]
    bind: String,

    /// Enable mDNS advertisement
    #[arg(long, default_value_t = true, env = "DSPRADIO_MDNS")]
    mdns: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "DSPRADIO_LOG")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize logging
    let level = match args.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .init();

    info!("🎛️  DSPradio Broker starting...");
    info!("   Port: {}", args.port);
    info!("   Bind: {}", args.bind);
    info!("   mDNS: {}", if args.mdns { "enabled" } else { "disabled" });

    // Create broker
    let broker = broker::Broker::new();

    // Start mDNS advertisement
    if args.mdns {
        match broker.start_mdns(args.port) {
            Ok(_) => info!("   mDNS: advertising on _dspradio._tcp"),
            Err(e) => tracing::warn!("   mDNS failed: {}", e),
        }
    }

    // Build API router
    let app = api::create_router(broker);

    // Start server
    let addr = format!("{}:{}", args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("🚀 Broker listening on http://{}", addr);
    info!("   API docs: http://{}/api/v1", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
