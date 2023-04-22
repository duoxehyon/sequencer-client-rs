use crossbeam_channel::unbounded;
use sequencer_client::feed_clients::RelayClients;
use std::sync::Arc;

use env_logger::Builder;
use log::LevelFilter;

#[tokio::main]
async fn main() {
    Builder::new()
        .filter_level(LevelFilter::Info) // Set log level to Info
        .write_style(env_logger::WriteStyle::Always) // Enable output to stdout
        .init();

    // Create a channel to receive messages from the feed client
    let (sender, receiver) = unbounded();

    // Create a new relay client and start background maintenance
    let relay_client = RelayClients::new("wss://nova.arbitrum.io/feed", 42170, 2, 1, sender)
        .expect("Failed to create relay client");
    RelayClients::start_reader(Arc::new(relay_client));

    let mut highest_seq_number: i64 = 0;

    loop {
        let data = receiver
            .recv()
            .expect("Failed to receive data from feed client");

        if highest_seq_number >= data.seq_num {
            continue;
        }

        highest_seq_number = data.seq_num;
        // let elapsed_time = data.time.elapsed();

        // info!("Received message, sequencer_number: {} | Took {:?}", data.seq_num, elapsed_time);
    }
}
