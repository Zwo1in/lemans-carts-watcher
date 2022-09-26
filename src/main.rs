use anyhow::Result;
use futures::{Sink, SinkExt, StreamExt};
use tokio::io::AsyncWriteExt;
use tokio_tungstenite::{connect_async, tungstenite::Error, tungstenite::Message};

use reqwest::header;

const AUTH: &str = "Basic bGVtYW5zb2xpbXBpYTowM2IxMzk3OS1jZTFlLTQ5Y2YtYjg4Yy0wYzQ4ZWRkOWYzNjg=";
const CONFIG_URL: &str = "https://backend.sms-timing.com/api/connectioninfo?type=modules";

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LemansConfig {
    access_token: String,
    client_key: String,
    service_address: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiveServerConfig {
    live_server_key: String,
    live_server_host: String,
    live_server_wss_port: usize,
}

async fn get_config() -> Result<LemansConfig> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_static(AUTH),
    );
    let config = reqwest::ClientBuilder::new()
        .default_headers(headers)
        .build()?
        .get(CONFIG_URL)
        .send()
        .await?
        .json()
        .await?;
    Ok(config)
}

async fn run_and_keep_alive<S: Sink<Message, Error = Error> + Unpin>(
    mut ws_write: S,
    server_key: String,
) -> Result<()> {
    ws_write
        .send(Message::Text(format!("START {}", server_key,)))
        .await?;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        ws_write.send(Message::Ping(vec![1])).await?;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let lemans_config = get_config().await?;
    let live_server_config_url = format!(
        "https://{}/api/livetiming/settings/{}?locale=en-US&styleId=&resourceId=&accessToken={}",
        lemans_config.service_address, lemans_config.client_key, lemans_config.access_token
    );
    let live_server_config: LiveServerConfig =
        reqwest::get(live_server_config_url).await?.json().await?;

    let wss_url = format!(
        "wss://{}:{}",
        live_server_config.live_server_host, live_server_config.live_server_wss_port
    );
    let wss_url = url::Url::parse(&wss_url).unwrap();

    let (ws_stream, _) = connect_async(wss_url).await?;

    let (ws_write, ws_read) = ws_stream.split();

    tokio::spawn(async {
        run_and_keep_alive(ws_write, live_server_config.live_server_key)
            .await
            .expect("Failed to run")
    });
    ws_read
        .for_each(|message| async {
            #[allow(clippy::match_single_binding)]
            match message.expect("Failed reading message") {
                // uncomment to see pongs
                // Message::Pong(_) => tokio::io::stdout()
                //     .write_all("Pong\n".as_bytes())
                //     .await
                //     .unwrap(),
                msg => tokio::io::stdout()
                    .write_all(&msg.into_data())
                    .await
                    .unwrap(),
            }
        })
        .await;

    Ok(())
}
