use biscuit_auth::{macros::biscuit, KeyPair};
use dropshot::{ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, ServerBuilder};
use reqwest::{multipart, Client};
use serde_json::json;
use std::env;
use std::sync::Arc;
use std::time::SystemTime;
use webshot::{auth::Auth, browser, ping, v1, ServerContext};

fn get_build() -> Option<String> {
    env::var("BUILD").ok()
}

fn generate_test_token(keypair: &KeyPair) -> String {
    let in_an_hour = SystemTime::now() + std::time::Duration::from_secs(60 * 60);

    let builder = biscuit!(
        r#"
        user("nobody");
        check if time($time), $time < {expiration};
        "#,
        expiration = in_an_hour,
    );

    let token = builder.build(keypair).unwrap();
    token.to_base64().unwrap()
}

struct UploadImageRequest {
    build: String,
    collection: String,
    snapshot: String,
    formula: String,
    payload: Vec<u8>,
}

async fn upload_image(req: UploadImageRequest) {
    let client = Client::new();

    let part = multipart::Part::bytes(req.payload)
        .file_name("image.png")
        .mime_str("image/png")
        .unwrap();

    let form = multipart::Form::new()
        .text("collection", req.collection)
        .text("snapshot", req.snapshot)
        .text("formula", req.formula)
        .part("payload", part);

    let url = format!(
        "https://app.urnerys.dev/api/v1/projects/webshot/builds/{}/images",
        req.build
    );

    let res = client.post(&url).multipart(form).send().await.unwrap();

    if !res.status().is_success() {
        panic!("Failed to upload image: {:?}", res.text().await);
    }
}

async fn test_render(base_url: &str, token: &str) {
    let payload = json!({
        "device": {
            "viewport": {
                "width": 1200,
                "height": 600
            },
            "scale": 2
        },
        "input": "<h1 style='color:red;'>Hello World"
    });

    let client = Client::new();

    let res = client
        .post(format!("{}/v1/render", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200, "Response status should be 200");

    let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(
        content_type, "image/png",
        "Content type should be image/png"
    );

    let image_data = res.bytes().await.unwrap().to_vec();
    assert!(!image_data.is_empty(), "Image data should not be empty");

    if let Some(build) = get_build() {
        upload_image(UploadImageRequest {
            build,
            collection: "End-to-End Tests/v1".to_string(),
            snapshot: "Render".to_string(),
            formula: "1200x600-scale:2".to_string(),
            payload: image_data,
        })
        .await;
    }
}

async fn test_capture(base_url: &str, token: &str) {
    let payload = json!({
        "device": {
            "viewport": {
                "width": 1200,
                "height": 600
            },
            "scale": 2
        },
        "input": "https://example.com",
        "target": {
            "kind": "viewport"
        }
    });

    let client = Client::new();

    let res = client
        .post(format!("{}/v1/capture", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200, "Response status should be 200");

    let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(
        content_type, "image/png",
        "Content type should be image/png"
    );

    let image_data = res.bytes().await.unwrap().to_vec();
    assert!(!image_data.is_empty(), "Image data should not be empty");

    if let Some(build) = get_build() {
        upload_image(UploadImageRequest {
            build,
            collection: "End-to-End Tests/v1".to_string(),
            snapshot: "Capture".to_string(),
            formula: "1200x600-scale:2".to_string(),
            payload: image_data,
        })
        .await;
    }
}

#[tokio::test]
async fn integration_test_suite() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info,chromiumoxide::handler=error")
        .try_init();

    let keypair = KeyPair::new();
    let auth = Auth {
        public_key: keypair.public(),
    };

    // Launch browser once for the whole suite
    let browser = browser::launch_browser()
        .await
        .expect("Failed to launch browser");

    let context = Arc::new(ServerContext { auth, browser });

    let mut api = ApiDescription::new();
    api.register(ping).unwrap();
    api.register(v1::render).unwrap();
    api.register(v1::capture).unwrap();

    let config = ConfigDropshot {
        bind_address: "127.0.0.1:0".parse().unwrap(),
        default_request_body_max_bytes: 1024 * 1024,
        ..Default::default()
    };

    let log = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    }
    .to_logger("webshot_test")
    .unwrap();

    let server = ServerBuilder::new(api, context, log)
        .config(config)
        .start()
        .unwrap();

    let port = server.local_addr().port();
    let base_url = format!("http://127.0.0.1:{}", port);
    let token = generate_test_token(&keypair);

    let server_task = tokio::spawn(async move {
        server.await.unwrap();
    });

    // Run tests sequentially
    test_render(&base_url, &token).await;
    test_capture(&base_url, &token).await;

    // Clean up
    server_task.abort();
}
