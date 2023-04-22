use base64;
use ethereum_types::{H160, U256};
use rlp::{self, DecoderError, Rlp};

use crate::types::L1IncomingMessageHeader;

const MAX_L2_MESSAGE_SIZE: usize = 256 * 1024;
const L1_MESSAGE_TYPE_L2_MESSAGE: i64 = 3;

/// For decoding "to"
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Create,
    Call(H160),
}

impl rlp::Decodable for Action {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        if rlp.is_empty() {
            if rlp.is_data() {
                Ok(Action::Create)
            } else {
                Err(DecoderError::RlpExpectedToBeData)
            }
        } else {
            Ok(Action::Call(rlp.as_val()?))
        }
    }
}

impl L1IncomingMessageHeader {
    pub fn decode(&self) -> Option<(H160, U256, Vec<u8>)> {
        if self.l2msg.len() > MAX_L2_MESSAGE_SIZE {
            return None;
        }

        if self.header.kind == L1_MESSAGE_TYPE_L2_MESSAGE {
            return base64::decode(self.l2msg.as_bytes())
                .ok()
                .and_then(|bytes| Self::parse_l2_message(bytes));
        }

        return None;
    }

    // 2 us faster
    fn parse_l2_message(data: Vec<u8>) -> Option<(H160, U256, Vec<u8>)> {
        if *data.get(0)? == 4 {
            let buf_after = data.get(1..)?;
            let pointer = *buf_after.get(0)?;
            // Regular transactions
            if pointer > 0x7f {
                // Legacy tx
                // Last val at 8
                let rlp_object = rlp::Rlp::new(&buf_after);
                // Offset at zero
                return Self::parse_single_tx(rlp_object, 0);
            } else if pointer == 1 {
                // Normal fee with access list
                // Last val at 10
                // Offset at 1 (EIP-2930)
                let rlp_object = rlp::Rlp::new(&buf_after[1..]);
                return Self::parse_single_tx(rlp_object, 1);
            } else if pointer == 2 {
                // Dynamic fee with access list
                // Last val at 11
                // Offset at 2 (EIP 1559)
                let rlp_object = rlp::Rlp::new(&buf_after[1..]);
                return Self::parse_single_tx(rlp_object, 2);
            }
        };
        return None;
    }

    fn parse_single_tx(data: Rlp, offset: usize) -> Option<(H160, U256, Vec<u8>)> {
        Some((
            data.val_at(offset + 3).ok()?,
            data.val_at(offset + 4).ok()?,
            data.val_at(offset + 5).ok()?,
        ))
    }
}
