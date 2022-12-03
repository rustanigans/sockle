use super::*;
use tungstenite::{stream::MaybeTlsStream, Error};

pub struct SimpleSockleClient
{
    pub(crate) socket: Option<tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>>
}

impl Default for SimpleSockleClient
{
    fn default() -> Self
    {
        Self::new()
    }
}

impl SimpleSockleClient
{
    pub fn new() -> Self
    {
        Self { socket: None }
    }

    pub(crate) fn set_non_blocking(&self, value: bool) -> Result<(), SimpleSockleError>
    {
        let socket = self.socket.as_ref().unwrap();
        match socket.get_ref()
        {
            MaybeTlsStream::Plain(s) =>
            {
                s.set_nonblocking(value)
                 .map_err(|x| SimpleSockleError::IoError(x))?
            }
            MaybeTlsStream::NativeTls(s) =>
            {
                s.get_ref()
                 .set_nonblocking(value)
                 .map_err(|x| SimpleSockleError::IoError(x))?
            }
            _ => unimplemented!("RustLs not supported")
        }
        Ok(())
    }

    pub(crate) fn set_timeout(&self, value: Option<Duration>) -> Result<(), SimpleSockleError>
    {
        let socket = self.socket.as_ref().unwrap();
        match socket.get_ref()
        {
            MaybeTlsStream::Plain(s) =>
            {
                s.set_read_timeout(value)
                 .map_err(|x| SimpleSockleError::IoError(x))?
            }
            MaybeTlsStream::NativeTls(s) =>
            {
                s.get_ref()
                 .set_read_timeout(value)
                 .map_err(|x| SimpleSockleError::IoError(x))?
            }
            _ => unimplemented!("RustLs not supported")
        }
        Ok(())
    }

    pub(crate) fn read_and_wrap_by_error_kind<F: Fn(std::io::ErrorKind) -> bool>(
        &mut self,
        f: F)
        -> Result<Option<String>, SimpleSockleError>
    {
        match self.read_message()
        {
            Ok(message) => Ok(Some(message)),
            Err(SimpleSockleError::SocketError(Error::Io(ee))) if f(ee.kind()) => Ok(None),
            Err(e) => Err(e)
        }
    }

    pub(crate) fn close_socket(&mut self, cf: Option<CloseFrame>) -> Result<(), SimpleSockleError>
    {
        use std::time::Instant;

        let socket = self.socket.as_mut().unwrap();
        log::debug!("Sending close frame");
        if socket.close(cf).is_err()
        {
            log::debug!("Send close frame failed, assumed already closed");
            self.socket = None;
            return Ok(());
        }

        log::debug!("Writing pending message until socket closed");
        let timeout = Instant::now() + Duration::from_secs(10);
        while !socket.write_pending().is_err() && timeout > Instant::now()
        {
            std::thread::yield_now();
        }

        self.socket = None;
        if timeout < Instant::now()
        {
            log::debug!("Socket not closed by server after close frame sent");
            Err(SimpleSockleError::SocketCloseTimeout)
        }
        else
        {
            Ok(())
        }
    }

    pub(crate) fn error_if_closed(&self) -> Result<(), SimpleSockleError>
    {
        if self.socket.is_none()
        {
            Err(SimpleSockleError::SocketDisconnected)
        }
        else
        {
            Ok(())
        }
    }

    pub(crate) fn read_message(&mut self) -> Result<String, SimpleSockleError>
    {
        let socket = self.socket.as_mut().unwrap();
        loop
        {
            match socket.read_message()
                        .map_err(SimpleSockleClient::map_error)?
            {
                Message::Text(t) => return Ok(t),
                Message::Binary(_) =>
                {
                    log::error!("Binary data not supported")
                }
                Message::Ping(_) =>
                {
                    log::debug!("Received ping.");
                }
                Message::Pong(_) =>
                {
                    log::debug!("Received pong.");
                }
                Message::Close(c) =>
                {
                    log::info!("Received close frame.");
                    if let Some(c) = c.as_ref()
                    {
                        log::info!(" Close reason: {} / {}", c.code, c.reason);
                    }
                    let _ = self.close_socket(c);
                    return Err(SimpleSockleError::SocketDisconnected);
                }
                Message::Frame(_) =>
                {
                    unreachable!()
                }
            }
        }
    }

    pub(crate) fn map_error(err: Error) -> SimpleSockleError
    {
        match err
        {
            Error::ConnectionClosed | Error::AlreadyClosed => SimpleSockleError::SocketDisconnected,
            e => SimpleSockleError::SocketError(e)
        }
    }
}
