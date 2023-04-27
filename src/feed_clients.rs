use crate::errors::*;
use crate::feed_client::*;
use crate::types::Tx;
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::error;
use log::*;
use std::thread;
use std::time::Duration;
use std::{
    sync::{Arc, Mutex},
    time,
};
use url::Url;

// For maintaining the sequencer feed clients
pub struct RelayClients {
    // All Clients
    clients: Mutex<Vec<RelayClient>>,
    // Max number of relay connections
    max_connections: usize,
    // To send transactions
    sender: Sender<Tx>,
    // Connection error receiver
    error_receiver: Receiver<ConnectionUpdate>,
    // Connection error sender
    error_sender: Sender<ConnectionUpdate>,
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
        max_connections: usize,
        init_connections: u8,
        sender: Sender<Tx>,
    ) -> Result<Self, RelayError> {
        let url = Url::parse(url).map_err(|_x| RelayError::InvalidUrl)?;

        let updates = unbounded();
        let mut connections: Vec<RelayClient> = Vec::new();

        for id in 0..init_connections {
            let mut conn = RelayClient::new(
                url.clone(),
                chain_id,
                id.into(),
                sender.clone(),
                updates.0.clone(),
            )?;
            match conn.start() {
                Ok(_) => (),
                Err(_) => {
                    return Err(RelayError::InitialConnectionError(
                        ConnectionError::RateLimited,
                    ))
                }
            };
            connections.push(conn);
        }

        Ok(RelayClients {
            clients: Mutex::new(connections),
            error_receiver: updates.1,
            error_sender: updates.0,
            sender,
            url,
            chain_id,
            max_connections,
        })
    }

    // Required to call after making a new instance
    pub fn start_reader(self: Arc<Self>) {
        let mut last_connected_time = time::SystemTime::now();
        let mut last_disconnected_time = time::SystemTime::now();

        let mut active_clients: Vec<bool> = {
            // Lock mutex and create vector of active client states
            let clients = self.clients.lock().unwrap();
            vec![true; clients.len()]
        };

        let mut total_active_clients = active_clients.len();
        let mut total_clients = total_active_clients;

        let max_clients = self.max_connections;
        let mut num_checks = 0;

        // Create a new thread that runs the background maintain loop
        tokio::spawn(async move {
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
                        let updated_id: usize = match id {
                            ConnectionUpdate::StoppedSendingFrames(d) => d,
                            ConnectionUpdate::Unknown(d) => d,
                        };

                        {
                            let bool = match active_clients.get_mut(updated_id) {
                                Some(d) => d,
                                None => panic!("This is not supposed to happen [feed_client/120]"),
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
                                if self.add_client(id).is_err() {
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
                            if self.add_client(total_clients).is_err() {
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
        });
    }

    // Adds a new client connection
    fn add_client(&self, id: usize) -> Result<(), RelayError> {
        let mut client = RelayClient::new(
            self.url.clone(),
            self.chain_id,
            id,
            self.sender.clone(),
            self.error_sender.clone(),
        )?;

        match client.start() {
            Ok(_) => (),
            Err(_) => {
                return Err(RelayError::InitialConnectionError(
                    ConnectionError::RateLimited,
                ))
            }
        };

        let mut clients = self.clients.lock().unwrap();

        if id == clients.len() {
            clients.push(client);
        } else {
            clients[id] = client;
        }

        Ok(())
    }
}
