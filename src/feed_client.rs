use crate::errors::{RelayError, ConnectionUpdate};
use crate::types::Root;
use crossbeam_channel::Sender;
use ethers::providers::StreamExt;
use log::*;
use tokio::task::JoinHandle;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use url::Url;

/// Sequencer Feed Client
pub struct RelayClient {
    // Socket connection to read from
    connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
    // For sending errors / disconnects
    connection_update: Sender<ConnectionUpdate>,
    // Sends Transactions
    sender: Sender<Root>,
    // Relay ID
    id: u32,
}

impl RelayClient {
    // Does not start the reader, only makes the websocket connection
    pub async fn new(
        url: Url,
        chain_id: u64,
        id: u32,
        sender: Sender<Root>,
        connection_update: Sender<ConnectionUpdate>,
    ) -> Result<Self, RelayError> {
        info!("Adding client | Client Id: {}", id);

        let key = tungstenite::handshake::client::generate_key();
        let host = url
            .host_str()
            .ok_or(RelayError::Msg("Invalid URL".to_owned()))?;

        let req = tungstenite::handshake::client::Request::builder()
            .method("GET")
            .uri(url.as_str())
            .header("Host", host)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", key)
            .header("Arbitrum-Feed-Client-Version", "2")
            .header("Arbitrum-Requested-Sequence-number", "0")
            .body(())?;

        let (socket, resp) = connect_async(req).await?;

        let chain_id_resp = resp
            .headers()
            .get("arbitrum-chain-id")
            .ok_or(RelayError::InvalidChainId)?
            .to_str()
            .unwrap_or_default();

        if chain_id_resp.parse::<u64>().unwrap_or_default()  != chain_id {
            return Err(RelayError::InvalidChainId);
        }

        Ok(Self {
            connection: socket,
            connection_update,
            sender,
            id,
        })
    }

    // Start the reader
    pub fn spawn(self) -> JoinHandle<()> {
        info!("Sequencer feed reader started | Client Id: {}", self.id);

        tokio::task::spawn(async move {
            match self.run().await {
                Ok(_) => (),
                Err(e) => error!("{}", e)
            }
        })
    }

    pub async fn run(mut self) -> Result<(), RelayError> {
        while let Some(msg) = self.connection.next().await {
            match msg {
                Ok(message) => {
                    let decoded_root: Root = match serde_json::from_slice(&message.into_data()) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
    
                    if self.sender.send(decoded_root).is_err() {
                        break; // we gracefully exit
                    }
                }
                Err(e) => {
                    self.connection_update
                        .send(ConnectionUpdate::StoppedSendingFrames(self.id))?;
                    error!("Connection closed with error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}
