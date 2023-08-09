use thiserror::Error;
use tokio::io;

pub type Result<T> = std::result::Result<T, RelayError>;

#[derive(Debug, Error)]
pub enum RelayError {
    #[error(transparent)]
    IO(#[from] io::Error),

    #[error(transparent)]
    UrlParse(#[from] url::ParseError),

    #[error(transparent)]
    HTTP(#[from] tungstenite::http::Error),

    #[error(transparent)]
    Tungstenite(#[from] tungstenite::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    SendError(#[from] crossbeam_channel::SendError<ConnectionUpdate>),

    #[error("Invalid Url")]
    InvalidUrl,

    #[error("Sequencer feed is not for the given chain id")]
    InvalidChainId,

    #[error("Relay Error {0}")]
    Msg(String),
}

#[derive(Debug)]
pub enum ConnectionUpdate {
    StoppedSendingFrames(u32),
    Unknown(u32),
}
