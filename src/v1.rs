use crate::{
    auth::{authorize_request, ValidBiscuit},
    browser::{configure_page, create_page, take_screenshot},
    ServerContext,
};
use biscuit_auth::macros::authorizer;
use chromiumoxide::cdp::browser_protocol::page::{
    CaptureScreenshotFormat, EventLifecycleEvent, SetLifecycleEventsEnabledParams,
};
use dropshot::{endpoint, Body, HttpError, RequestContext, TypedBody};
use futures::StreamExt;
use http::{header, Response, StatusCode};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use url::Url;

#[derive(Deserialize, JsonSchema)]
pub struct Device {
    pub viewport: Viewport,
    pub scale: Option<f64>,
    #[serde(rename = "extraHTTPHeaders")]
    pub extra_http_headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize, JsonSchema)]
pub struct RenderRequest {
    pub device: Device,
    pub input: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind")]
pub enum Target {
    #[serde(rename = "viewport")]
    Viewport,
    #[serde(rename = "page")]
    Page,
    #[serde(rename = "element")]
    Element { locator: String },
}

#[derive(Deserialize, JsonSchema)]
pub struct CaptureRequest {
    pub device: Device,
    pub input: Url,
    pub target: Target,
}

#[endpoint {
    method = POST,
    path = "/v1/render",
}]
pub async fn render(
    rqctx: RequestContext<Arc<ServerContext>>,
    biscuit: ValidBiscuit,
    body: TypedBody<RenderRequest>,
) -> Result<Response<Body>, HttpError> {
    let authorizer = authorizer!(
        r#"
            time({time});
            operation("render");

            allow if user($u);
        "#,
        time = std::time::SystemTime::now()
    );

    let user = authorize_request(biscuit.0, authorizer)?;
    tracing::info!("Render Request from user:{}", user);

    let browser = &rqctx.context().browser;

    let (context_id, page) = create_page(browser).await?;

    let result: Result<Vec<u8>, HttpError> = async {
        let body_val = body.into_inner();
        configure_page(&page, &body_val.device).await?;

        page.execute(
            SetLifecycleEventsEnabledParams::builder()
                .enabled(true)
                .build()
                .map_err(|e| {
                    tracing::error!("Failed to build SetLifecycleEventsEnabledParams: {:?}", e);
                    HttpError::for_internal_error("Failed to enable lifecycle events".to_string())
                })?,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to enable lifecycle events: {:?}", e);
            HttpError::for_internal_error("Failed to enable lifecycle events".to_string())
        })?;

        let mut events = page
            .event_listener::<EventLifecycleEvent>()
            .await
            .map_err(|e| {
                tracing::error!("Failed to create event listener: {:?}", e);
                HttpError::for_internal_error("Failed to create event listener".to_string())
            })?;

        page.set_content(body_val.input).await.map_err(|e| {
            tracing::error!("Failed to set content: {:?}", e);
            HttpError::for_internal_error("Failed to set content".to_string())
        })?;

        let wait_result = timeout(Duration::from_secs(10), async {
            while let Some(event) = events.next().await {
                if event.name == "load" {
                    break;
                }
            }
        })
        .await;

        if wait_result.is_err() {
            tracing::warn!("Timeout waiting for load in render, proceeding with screenshot");
        }

        take_screenshot(&page, &Target::Viewport)
            .await
            .map_err(|e| {
                tracing::error!("Screenshot failed: {:?}", e);
                HttpError::for_internal_error("Screenshot failed".to_string())
            })
    }
    .await;

    if let Err(e) = page.close().await {
        tracing::warn!("Failed to close page: {:?}", e);
    }
    if let Err(e) = browser.dispose_browser_context(context_id).await {
        tracing::warn!("Failed to dispose browser context: {:?}", e);
    }

    let img = result?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(img))
        .map_err(|_| HttpError::for_internal_error("Response builder failed".to_string()))
}

#[endpoint {
    method = POST,
    path = "/v1/capture",
}]
pub async fn capture(
    rqctx: RequestContext<Arc<ServerContext>>,
    biscuit: ValidBiscuit,
    body: TypedBody<CaptureRequest>,
) -> Result<Response<Body>, HttpError> {
    let req = body.into_inner();

    let scheme = req.input.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(HttpError::for_bad_request(
            None,
            "Invalid URL scheme".to_string(),
        ));
    }

    let hostname = match req.input.host_str() {
        Some(h) if !h.is_empty() => h,
        _ => return Err(HttpError::for_bad_request(None, "Bad URL".to_string())),
    };

    let authorizer = authorizer!(
        r#"
            time({time});
            operation("capture");
            hostname({hostname});

            allow if user($u);
        "#,
        time = std::time::SystemTime::now(),
        hostname = hostname
    );

    let user = authorize_request(biscuit.0, authorizer)?;
    tracing::info!(
        "Capture Request from user:{} for hostname:{}",
        user,
        hostname
    );

    let browser = &rqctx.context().browser;

    let (context_id, page) = create_page(browser).await?;

    let result: Result<Vec<u8>, HttpError> = async {
        configure_page(&page, &req.device).await?;

        page.execute(
            SetLifecycleEventsEnabledParams::builder()
                .enabled(true)
                .build()
                .map_err(|e| {
                    tracing::error!("Failed to build SetLifecycleEventsEnabledParams: {:?}", e);
                    HttpError::for_internal_error("Failed to enable lifecycle events".to_string())
                })?,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to enable lifecycle events: {:?}", e);
            HttpError::for_internal_error("Failed to enable lifecycle events".to_string())
        })?;

        let mut events = page
            .event_listener::<EventLifecycleEvent>()
            .await
            .map_err(|e| {
                tracing::error!("Failed to create event listener: {:?}", e);
                HttpError::for_internal_error("Failed to create event listener".to_string())
            })?;

        let res = page.goto(req.input.as_str()).await;

        if res.is_ok() {
            let wait_result = timeout(Duration::from_secs(10), async {
                while let Some(event) = events.next().await {
                    if event.name == "networkIdle" {
                        break;
                    }
                }
            })
            .await;

            if wait_result.is_err() {
                tracing::warn!(
                    "Timeout waiting for networkIdle in capture, proceeding with screenshot"
                );
            }

            let image = take_screenshot(&page, &req.target).await;

            if let Ok(img) = image {
                return Ok(img);
            }
        }

        tracing::error!(url = %req.input, "Capture failed");

        if let Ok(fallback_img) = page
            .screenshot(
                chromiumoxide::page::ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Jpeg)
                    .quality(10)
                    .build(),
            )
            .await
        {
            use base64::{engine::general_purpose, Engine as _};
            let base64_str = general_purpose::STANDARD.encode(&fallback_img);
            tracing::info!(fallback_image_base64 = %base64_str, "Fallback debug screenshot");
        }

        Err(HttpError::for_internal_error("Capture failed".to_string()))
    }
    .await;

    if let Err(e) = page.close().await {
        tracing::warn!("Failed to close page: {:?}", e);
    }
    if let Err(e) = browser.dispose_browser_context(context_id).await {
        tracing::warn!("Failed to dispose browser context: {:?}", e);
    }

    let img = result?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(img))
        .map_err(|_| HttpError::for_internal_error("Response builder failed".to_string()))
}
