use dropshot::{ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, ServerBuilder};
use std::sync::Arc;
use webshot::{auth::Auth, browser, ping, v1, ServerContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("info,chromiumoxide::handler=error")
            }),
        )
        .init();

    let auth = Auth::from_env()?;
    let browser = browser::launch_browser().await?;

    let context = Arc::new(ServerContext { auth, browser });

    let mut api = ApiDescription::new();
    api.register(ping).unwrap();
    api.register(v1::render).unwrap();
    api.register(v1::capture).unwrap();

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()?;

    let config = ConfigDropshot {
        bind_address: format!("0.0.0.0:{}", port).parse()?,
        default_request_body_max_bytes: 1024 * 1024,
        ..Default::default()
    };

    let log = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    }
    .to_logger("webshot")
    .unwrap();

    let server = ServerBuilder::new(api, context, log)
        .config(config)
        .start()
        .unwrap();

    tracing::info!("Listening on port {}", port);
    server.await?;

    Ok(())
}
