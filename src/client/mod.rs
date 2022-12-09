use anyhow::Result;
use std::time::Duration;
use tungstenite::{protocol::CloseFrame, Message};
use url::Url;

mod simple_sockle_client;

use crate::SimpleSockleError;
pub use simple_sockle_client::SimpleSockleClient;

pub trait SockleClient
{
    /// Connects socket to url
    ///
    /// Can be called to reconnect if closed.
    /// Use close method to ensure it's closed first.
    fn connect(&mut self, url: &str) -> Result<()>;
    /// Writes a string message to the socket
    fn write(&mut self, msg: String) -> Result<()>;
    /// Reads if possible, return Ok(None) if not
    fn try_read(&mut self) -> Result<Option<String>>;
    /// Reads and blocks until a message is returned
    fn read(&mut self) -> Result<String>;
    /// Reads and blocks for timeout period, returning Ok(None) on timeout
    fn read_timeout(&mut self, timeout: Duration) -> Result<Option<String>>;
    /// Closes the socket connection, returns Ok(()) if already closed
    fn close(&mut self) -> Result<()>;
    /// Sends a ping
    fn ping(&mut self) -> Result<()>;
}

impl SockleClient for SimpleSockleClient
{
    fn connect(&mut self, url: &str) -> Result<()>
    {
        log::info!("Connecting socket ({url})");

        if self.error_if_closed().is_ok()
        {
            return Err(SimpleSockleError::SocketConnected.into());
        }

        let url = Url::parse(url).map_err(|e| SimpleSockleError::InvalidUrl(e.to_string()))?;

        let socket = tungstenite::connect(url).map_err(SimpleSockleClient::map_error)?
                                              .0;
        self.socket = Some(socket);

        log::info!("Connected");
        Ok(())
    }

    fn write(&mut self, msg: String) -> Result<()>
    {
        self.error_if_closed()?;
        Ok(self.socket
               .as_mut()
               .unwrap()
               .write_message(Message::Text(msg))
               .map_err(SimpleSockleClient::map_error)?)
    }

    fn try_read(&mut self) -> Result<Option<String>>
    {
        self.error_if_closed()?;
        self.set_non_blocking(true)?;

        let result = self.read_and_wrap_by_error_kind(|x| x == std::io::ErrorKind::WouldBlock);

        if result.is_ok()
        {
            self.set_non_blocking(false)?;
        }
        Ok(result?)
    }

    fn read(&mut self) -> Result<String>
    {
        self.error_if_closed()?;

        Ok(self.read_message()?)
    }

    fn read_timeout(&mut self, timeout: Duration) -> Result<Option<String>>
    {
        self.error_if_closed()?;
        self.set_timeout(Some(timeout))?;

        // Unix returns WouldBlock, windows returns TimedOut
        use std::io::ErrorKind::{TimedOut, WouldBlock};
        let result = self.read_and_wrap_by_error_kind(|x| matches!(x, WouldBlock | TimedOut));
        if result.is_ok()
        {
            self.set_timeout(None)?;
        }
        Ok(result?)
    }

    fn close(&mut self) -> Result<()>
    {
        if self.error_if_closed().is_err()
        {
            log::debug!("Attempted close socket on already closed socket. Ignoring");
            return Ok(());
        }
        log::info!("Closing socket");
        let result = self.close_socket(Some(CloseFrame {
            code: tungstenite::protocol::frame::coding::CloseCode::Normal,
            reason: "Client requested close".into(),
        }))?;
        log::info!("Socket Closed");

        Ok(result)
    }

    fn ping(&mut self) -> Result<()>
    {
        self.error_if_closed()?;
        self.socket
            .as_mut()
            .unwrap()
            .write_message(Message::Ping(vec![0]))?;
        Ok(())
    }
}
