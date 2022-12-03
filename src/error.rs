#[derive(thiserror::Error, Debug)]
pub enum SimpleSockleError
{
    #[error("Error parsing url used to connect: {0}")]
    InvalidUrl(String),
    #[error("Attempted operation on closed socket")]
    SocketDisconnected,
    #[error("Attempted connect on open socket")]
    SocketConnected,
    #[error("Error on underlying socket: {0}")]
    SocketError(tungstenite::Error),
    #[error("IO Error on underlying socket: {0}")]
    IoError(std::io::Error),
    #[error("Timeout while trying to close socket")]
    SocketCloseTimeout
}
