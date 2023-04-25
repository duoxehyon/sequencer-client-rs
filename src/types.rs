use ethereum_types::{H160, U256};
use ethers::types::Transaction;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;
use std::time::Instant;

/// Will be sent across after decoding
#[derive(Debug, Clone)]
pub struct Tx {
    // Message block num
    pub seq_num: i64,
    // Time at recieve
    pub time: Instant,
    // Tx   (to, value, data)
    pub tx: (H160, U256, Vec<u8>),
    pub l2_tx: Transaction,
}

/*
    Serde derive types
*/
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub version: i64,
    pub messages: Vec<BroadcastFeedMessage>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BroadcastFeedMessage {
    pub sequence_number: i64,
    pub message: MessageWithMetadata,
    pub signature: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageWithMetadata {
    pub message: L1IncomingMessageHeader,
    pub delayed_messages_read: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L1IncomingMessageHeader {
    pub header: Header,
    #[serde(rename = "l2Msg")]
    pub l2msg: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub kind: i64,
    pub sender: String,
    pub block_number: i64,
    pub timestamp: i64,
    pub request_id: Value,
    pub base_fee_l1: Value,
}
