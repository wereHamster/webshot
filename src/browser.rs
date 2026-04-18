use crate::v1::{Device, Target};
use chromiumoxide::{
    cdp::browser_protocol::browser::BrowserContextId,
    cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams,
    cdp::browser_protocol::network::{Headers, SetExtraHttpHeadersParams},
    cdp::browser_protocol::page::CaptureScreenshotFormat,
    cdp::browser_protocol::target::{CreateBrowserContextParams, CreateTargetParams},
    Browser, BrowserConfig, Page,
};
use dropshot::HttpError;
use futures::StreamExt;

pub async fn launch_browser() -> Result<Browser, Box<dyn std::error::Error>> {
    // chromiumoxide comes with a set of sensible default arguments. We add a
    // few more specific ones that are useful for a screenshot service like ours.
    let mut browser_config = BrowserConfig::builder().args(vec![
        "hide-scrollbars",
        "mute-audio",
        // --
        // The options below improve font rendering.
        "font-render-hinting=none",
        "disable-font-subpixel-positioning",
    ]);

    if let Ok(chrome_path) = std::env::var("CHROME") {
        tracing::info!("Using CHROME executable: {}", chrome_path);
        browser_config = browser_config.chrome_executable(chrome_path);
    }

    let (browser, mut handler) = Browser::launch(browser_config.build()?).await?;
    tokio::spawn(async move { while handler.next().await.is_some() {} });

    Ok(browser)
}

pub(crate) async fn create_page(browser: &Browser) -> Result<(BrowserContextId, Page), HttpError> {
    let context_id = browser
        .create_browser_context(CreateBrowserContextParams::builder().build())
        .await
        .map_err(|e| {
            tracing::error!("Failed to create browser context: {:?}", e);
            HttpError::for_internal_error("Failed to create browser context".to_string())
        })?;

    let target_params = match CreateTargetParams::builder()
        .url("about:blank")
        .browser_context_id(context_id.clone())
        .build()
    {
        Ok(params) => params,
        Err(e) => {
            tracing::error!("Failed to build CreateTargetParams: {:?}", e);
            let _ = browser.dispose_browser_context(context_id).await;
            return Err(HttpError::for_internal_error(
                "Failed to create page".to_string(),
            ));
        }
    };

    match browser.new_page(target_params).await {
        Ok(page) => Ok((context_id, page)),
        Err(e) => {
            tracing::error!("Failed to create page: {:?}", e);
            let _ = browser.dispose_browser_context(context_id).await;
            Err(HttpError::for_internal_error(
                "Failed to create page".to_string(),
            ))
        }
    }
}

pub(crate) async fn take_screenshot(
    page: &Page,
    target: &Target,
) -> chromiumoxide::error::Result<Vec<u8>> {
    match target {
        Target::Viewport => {
            page.screenshot(
                chromiumoxide::page::ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .build(),
            )
            .await
        }
        Target::Page => {
            page.screenshot(
                chromiumoxide::page::ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .full_page(true)
                    .build(),
            )
            .await
        }
        Target::Element { locator } => {
            let elem = page.find_element(locator.as_str()).await?;
            let elem = elem.scroll_into_view().await?;
            let bounding_box = elem.bounding_box().await?;
            let metrics = page.layout_metrics().await?;
            let viewport = metrics.css_layout_viewport;

            let clip = chromiumoxide::cdp::browser_protocol::page::Viewport {
                x: viewport.page_x as f64 + bounding_box.x,
                y: viewport.page_y as f64 + bounding_box.y,
                width: bounding_box.width,
                height: bounding_box.height,
                scale: 1.0,
            };

            page.screenshot(
                chromiumoxide::page::ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .clip(clip)
                    .capture_beyond_viewport(true)
                    .build(),
            )
            .await
        }
    }
}

pub(crate) async fn configure_page(page: &Page, device: &Device) -> Result<(), HttpError> {
    page.execute(
        SetDeviceMetricsOverrideParams::builder()
            .width(device.viewport.width as i64)
            .height(device.viewport.height as i64)
            .device_scale_factor(device.scale.unwrap_or(1.0))
            .mobile(false)
            .build()
            .map_err(|e| {
                tracing::error!("Failed to build SetDeviceMetricsOverrideParams: {:?}", e);
                HttpError::for_internal_error("Failed to configure viewport".to_string())
            })?,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to configure viewport: {:?}", e);
        HttpError::for_internal_error("Failed to configure viewport".to_string())
    })?;

    if let Some(headers) = &device.extra_http_headers {
        let headers_val = serde_json::to_value(headers).unwrap_or(serde_json::Value::Null);
        page.execute(
            SetExtraHttpHeadersParams::builder()
                .headers(Headers::new(headers_val))
                .build()
                .map_err(|e| {
                    tracing::error!("Failed to build SetExtraHttpHeadersParams: {:?}", e);
                    HttpError::for_internal_error(
                        "Failed to configure extra HTTP headers".to_string(),
                    )
                })?,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to configure extra HTTP headers: {:?}", e);
            HttpError::for_internal_error("Failed to configure extra HTTP headers".to_string())
        })?;
    }

    Ok(())
}
