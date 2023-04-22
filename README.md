# Sequencer-Client  (🚧)
Pure Rust implementation of Arbitrum sequencer feed reader with built-in transaction decoding and MEV features

## Design Goal
This Rust implementation is designed for a high number of concurrent connections and faster performance than the original Go implementation, with MEV-specific features.

## Quick Start
To use this sequencer-client, you'll need Tokio as your main runtime.

Here is a basic example.
```Rust
use crossbeam_channel::unbounded;
use sequencer_client::feed_clients::RelayClients;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Create a channel to receive messages from the feed client
    let (sender, receiver) = unbounded();

    // Create a new relay client and start reader + connection maintainer
    let relay_client = RelayClients::new("wss://nova.arbitrum.io/feed", 42170, 2, 1, sender)
        .expect("Failed to create relay client");
    RelayClients::start_reader(Arc::new(relay_client));

    // To prevent duplicate messages
    let mut highest_seq_number: i64 = 0;

    loop {
        let data = receiver.recv().expect("Failed to receive data from feed client");

        if highest_seq_number >= data.seq_num {
            continue;
        }

        highest_seq_number = data.seq_num;
        let elapsed_time = data.time.elapsed();
        info!("Received message, sequencer_number: {} | Took {:?}", data.seq_num, elapsed_time);
    }
}

```

## Status
Currently, the sequencer-reader does not have full transaction decoding and only includes the MEV specific parts

## License
This repo is licensed under the MIT license.
