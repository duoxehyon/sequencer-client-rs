use crate::errors::*;
use crate::feed_client::*;
use crate::types::Root;
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::error;
use log::*;
use std::collections::HashMap;
use std::thread;
use std::time;
use std::time::Duration;
use tokio::task::JoinHandle;
use url::Url;

// For maintaining the sequencer feed clients
pub struct RelayClients {
    // All Clients
    clients: HashMap<u32, JoinHandle<()>>,
    // Connection error receiver
    error_receiver: Receiver<ConnectionUpdate>,
    // Connection error sender
    error_sender: Sender<ConnectionUpdate>,
    // To send transactions
    sender: Sender<Root>,
    // Max number of relay connections
    max_connections: u32,
    // Connection Url
    url: Url,
    // Chain id
    chain_id: u64,
}

impl RelayClients {
    // Does not start the reader, only makes the initial connections
    pub fn new(
        url: &str,
        chain_id: u64,
        max_connections: u32,
        init_connections: u8,
        sender: Sender<Root>,
    ) -> Result<Self, RelayError> {
        let url = Url::parse(url).map_err(|_x| RelayError::InvalidUrl)?;

        let updates = unbounded();
        let mut connections: HashMap<u32, JoinHandle<()>> = HashMap::new();

        for id in 0..init_connections {
            let conn = RelayClient::new(
                url.clone(),
                chain_id,
                id.into(),
                sender.clone(),
                updates.0.clone(),
            )?;

            connections.insert(id.into(), conn.spawn());
        }

        Ok(RelayClients {
            clients: connections,
            error_receiver: updates.1,
            error_sender: updates.0,
            sender,
            url,
            chain_id,
            max_connections,
        })
    }

    // Required to call after making a new instance
    pub async fn start_reader(mut self) {
        let mut last_connected_time = time::SystemTime::now();
        let mut last_disconnected_time = time::SystemTime::now();

        let mut active_clients: Vec<bool> = vec![true; self.clients.len()];

        let mut total_active_clients: u32 = active_clients.len().try_into().unwrap();
        let mut total_clients = total_active_clients;

        let max_clients = self.max_connections;
        let mut num_checks = 0;

        loop {
            // Wait for 1 second before checking the connections again
            thread::sleep(Duration::from_secs(1));
            num_checks += 1;

            if num_checks > 40 {
                info!(
                    "Connections AC/TC: {}/{} | ratio: {:.2}",
                    total_active_clients,
                    total_clients,
                    total_active_clients as f32 / total_clients as f32
                );
                num_checks = 0;
            }

            // Check for any updates to the connections
            match self.error_receiver.try_recv() {
                Ok(id) => {
                    let updated_id: u32 = match id {
                        ConnectionUpdate::StoppedSendingFrames(d) => d,
                        ConnectionUpdate::Unknown(d) => d,
                    };

                    {
                        let bool = match active_clients.get_mut(updated_id as usize) {
                            Some(d) => d,
                            None => panic!("This is not supposed to happen"),
                        };

                        *bool = false;
                        total_active_clients -= 1;
                    }

                    last_disconnected_time = time::SystemTime::now();
                    warn!("Client disconnected | Client Id: {}", updated_id);
                }
                Err(_) => {
                    // No message was received, check the connections
                    let now = std::time::SystemTime::now();
                    let elapsed_connected =
                        now.duration_since(last_connected_time).unwrap().as_secs();
                    let elapsed_disconnect = now
                        .duration_since(last_disconnected_time)
                        .unwrap()
                        .as_secs();

                    let mut up_coming_connection = None;
                    // Iterate over all connections
                    for (id, is_connected) in active_clients.iter_mut().enumerate() {
                        if !*is_connected && elapsed_connected > 70 && elapsed_disconnect > 70 {
                            // This connection is not connected and it's been more than 70 seconds since the last connect / disconnect event
                            up_coming_connection = Some(id);
                            // Adding the connection
                            if self.add_or_replace_client(id as u32).is_err() {
                                last_disconnected_time = time::SystemTime::now();
                                error!("Failed to add client");
                                break;
                            }
                            *is_connected = true;

                            total_active_clients += 1;
                            last_connected_time = now;
                            break;
                        }
                    }

                    // If all connections are active and max connections is less than current "total connections", we add
                    if up_coming_connection.is_none()
                        && elapsed_connected > 70
                        && elapsed_disconnect > 70
                        && max_clients > total_clients
                    {
                        // A new connection needs to be created
                        if self.add_or_replace_client(total_clients).is_err() {
                            last_disconnected_time = time::SystemTime::now();
                            error!("Failed to add client");
                            continue;
                        }

                        active_clients.push(true);
                        total_active_clients += 1;
                        total_clients += 1;
                        last_connected_time = time::SystemTime::now();
                    }
                }
            }
        }
    }

    // Adds a new client connection
    fn add_or_replace_client(&mut self, id: u32) -> Result<(), RelayError> {
        let client = RelayClient::new(
            self.url.clone(),
            self.chain_id,
            id,
            self.sender.clone(),
            self.error_sender.clone(),
        )?;

        let handle = client.spawn();
        self.clients.insert(id, handle);

        Ok(())
    }
}
