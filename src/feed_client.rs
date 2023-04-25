use crate::errors::*;
use crate::types::Root;
use crate::types::Tx;
use crossbeam_channel::{unbounded, Receiver, Sender};
use ethers::types::Transaction;
use log::error;
use log::*;
use std::time::Instant;
use std::{
    error::Error,
    net::TcpStream,
    sync::{Arc, Mutex},
};

use tungstenite::{stream::MaybeTlsStream, WebSocket};
use url::Url;

/// Sequencer Feed Client
pub struct RelayClient {
    // Socket connection to read from
    connection: Arc<Mutex<WebSocket<MaybeTlsStream<TcpStream>>>>,
    // Sends Transactions
    sender: Sender<Tx>,
    // For Stopping the reader
    receiver: (Sender<()>, Receiver<()>),
    // For sending errors / disconnects
    connection_update: Sender<ConnectionUpdate>,
    // Relay ID
    id: usize,
}

impl RelayClient {
    // Does not start the reader, only makes the websocket connection
    pub fn new(
        url: Url,
        chain_id: u64,
        id: usize,
        sender: Sender<Tx>,
        connection_update: Sender<ConnectionUpdate>,
    ) -> Result<Self, RelayError> {
        info!("Adding client | Client Id: {}", id);

        let key = tungstenite::handshake::client::generate_key();
        let host = url
            .host_str()
            .ok_or(RelayError::InitialConnectionError(ConnectionError::Unknown))?;

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
            .body(())
            .map_err(|_| RelayError::InitialConnectionError(ConnectionError::RequestTimeOut))?;

        let (socket, resp) = match tungstenite::connect(req) {
            Ok(d) => d,
            Err(_) => {
                return Err(RelayError::InitialConnectionError(
                    ConnectionError::RateLimited,
                ))
            }
        }; // Panic at the start

        let chain_id_resp = resp
            .headers()
            .get("arbitrum-chain-id")
            .ok_or(RelayError::InitialConnectionError(ConnectionError::Unknown))?
            .to_str()
            .unwrap_or_default();

        if chain_id_resp.parse::<u64>().unwrap_or_default() != chain_id {
            return Err(RelayError::InitialConnectionError(
                ConnectionError::InvalidChainId,
            ));
        }

        let receiver = unbounded();

        Ok(Self {
            connection: Arc::new(Mutex::new(socket)),
            connection_update,
            sender,
            receiver,
            id,
        })
    }

    // Sends a signal to the reader to stop reading.
    pub fn disconnect(&self) -> Result<(), Box<dyn Error>> {
        self.receiver.0.send(())?;

        let mut connection = self.connection.lock().unwrap();
        connection.close(None)?;

        Ok(())
    }

    // Start the reader
    pub fn start(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Sequencer feed reader started | Client Id: {}", self.id);

        let receive_end = self.receiver.1.clone();
        let client = self.connection.clone();
        let sender = self.sender.clone();
        let update_sender = self.connection_update.clone();
        let id = self.id;

        tokio::spawn(async move {
            let mut connection = client.lock().unwrap();
            let mut read = 0;

            loop {
                match connection.read_message() {
                    Ok(message) => {
                        // skip Intital frames
                        if read < 4 {
                            read += 1;
                            continue;
                        }

                        // for benchmarking / disconnecting bad connections
                        let now = Instant::now();

                        let decoded_root: Root = match serde_json::from_slice(&message.into_data())
                        {
                            Ok(d) => d,
                            Err(_) => continue,
                        };

                        // first tx only
                        let decoded_tx = match decoded_root.messages[0].message.message.decode() {
                            Some(d) => d,
                            None => continue,
                        };
                        let l2_bytes =
                            base64::decode(&decoded_root.messages[0].message.message.l2msg)
                                .unwrap();
                        let l2_tx: Transaction =
                            ethers::utils::rlp::decode(&l2_bytes[1..]).unwrap();

                        let tx = Tx {
                            time: now,
                            seq_num: decoded_root.messages[0].sequence_number,
                            tx: decoded_tx,
                            l2_tx,
                        };

                        sender.send(tx).unwrap();
                    }
                    Err(e) => {
                        update_sender
                            .send(ConnectionUpdate::StoppedSendingFrames(id))
                            .unwrap();
                        error!("Connection closed with error: {}", e);
                        break;
                    }
                }

                if receive_end.try_recv().is_ok() {
                    break;
                }
            }
        });

        Ok(())
    }
}

