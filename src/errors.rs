use std::fmt;

#[derive(Debug)]
pub enum RelayError {
    InitialConnectionError(ConnectionError),
    SuddenDisconnectError(ConnectionError),
    InvalidUrl,
}

#[derive(Debug)]
pub enum ConnectionError {
    RequestTimeOut,
    RateLimited,
    InvalidChainId,
    Unknown,
}

#[derive(Debug)]
pub enum ConnectionUpdate {
    StoppedSendingFrames(usize),
    Unknown(usize),
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConnectionError::RequestTimeOut => write!(f, "Connection timed out"),
            ConnectionError::RateLimited => write!(f, "Connection rate limited"),
            ConnectionError::Unknown => write!(f, "Unknown connection error"),
            ConnectionError::InvalidChainId => {
                write!(f, "Sequencer feed is not for the given chain id")
            }
        }
    }
}

impl fmt::Display for RelayError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RelayError::InitialConnectionError(e) => {
                write!(f, "Error connecting to relay: {}", e)
            }
            RelayError::SuddenDisconnectError(e) => {
                write!(f, "Unexpected disconnect from relay: {}", e)
            }
            RelayError::InvalidUrl => {
                write!(f, "Invalid url")
            }
        }
    }
}
