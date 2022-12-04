use anyhow::Result;
use std::{collections::VecDeque,
          net::{TcpListener, TcpStream},
          sync::{mpsc::TryRecvError, Arc},
          time::{Duration, Instant}};
use tungstenite::{protocol::{frame::coding::CloseCode, CloseFrame},
                  Message};

pub trait SockleServer
{
    /// Spawns a thread and listens on given ip/port
    fn listen<F: Fn(String, Box<dyn Fn(String)>) -> Result<()> + Send + Sync + 'static>(
        &mut self,
        listen_address: &str,
        on_message: F)
        -> Result<()>;

    /// Sends a message to all connected clients
    fn send(&self, msg: String);

    /// Closes all connections and stops listening
    ///
    /// Blocks until thread has ended
    fn shutdown(&self) -> Result<()>;

    /// Number of client connections
    fn connection_count(&self) -> usize;
}

pub enum SockleServerMessage
{
    Send(String),
    Shutdown
}

pub struct SimpleSockleServer
{
    thread_ctrl:    Option<std::sync::mpsc::Sender<()>>,
    thread_senders: Arc<std::sync::Mutex<Vec<std::sync::mpsc::Sender<SockleServerMessage>>>>
}

impl Default for SimpleSockleServer
{
    fn default() -> Self
    {
        Self::new()
    }
}

impl SimpleSockleServer
{
    pub fn new() -> Self
    {
        SimpleSockleServer { thread_ctrl:    None,
                             thread_senders: Default::default() }
    }
}

pub type OnMessageFn = Arc<dyn Fn(String, Box<dyn Fn(String)>) -> Result<()> + Send + Sync>;

pub struct Conn
{
    socket:     tungstenite::WebSocket<TcpStream>,
    ctrl:       std::sync::mpsc::Receiver<SockleServerMessage>,
    on_message: OnMessageFn
}

impl Conn
{
    fn new(socket: tungstenite::WebSocket<TcpStream>,
           ctrl: std::sync::mpsc::Receiver<SockleServerMessage>,
           on_message: OnMessageFn)
           -> Conn
    {
        Self { socket,
               ctrl,
               on_message }
    }

    fn on_accept(mut self)
    {
        if let Err(e) = self.socket
                            .get_ref()
                            .set_read_timeout(Some(Duration::from_millis(15)))
        {
            log::error!("Unable to set timeout on incoming socket: {e}");
            return;
        }

        loop
        {
            match self.socket.read_message()
            {
                Ok(msg) =>
                {
                    if !self.on_message(msg)
                    {
                        return;
                    }
                }
                Err(tungstenite::error::Error::Io(e))
                    if matches!(e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) =>
                {}
                Err(e) =>
                {
                    log::error!("Error on client socket: {e}");
                    self.close_socket(Some(CloseFrame { code:   CloseCode::Error,
                                                        reason: e.to_string().into() }));
                    return;
                }
            }
            match self.ctrl.try_recv()
            {
                Ok(SockleServerMessage::Send(msg)) =>
                {
                    log::debug!("Received Send ctrl message on socket, writing to client");
                    if let Err(e) = self.socket.write_message(Message::Text(msg))
                    {
                        log::error!("Unable to write broadcast to socket: {e}");
                        return;
                    }
                }
                Ok(SockleServerMessage::Shutdown) =>
                {
                    log::info!("Shutting down, closing a client socket");
                    self.close_socket(Some(CloseFrame { code:   CloseCode::Normal,
                                                        reason: "Server Shutdown".into() }));
                    return;
                }
                Err(TryRecvError::Disconnected) =>
                {
                    log::warn!("Client ctrl channel disconnected, closing client socket");
                    self.close_socket(Some(CloseFrame { code:   CloseCode::Normal,
                                                        reason: "Server Error".into() }));
                    return;
                }
                Err(TryRecvError::Empty) => std::thread::yield_now()
            }
        }
    }

    fn on_message(&mut self, msg: Message) -> bool
    {
        match msg
        {
            Message::Text(message) =>
            {
                let q = Arc::new(std::sync::Mutex::new(VecDeque::new()));
                let q2 = q.clone();
                if let Err(e) =
                    (self.on_message)(message, Box::new(move |s| q2.lock().unwrap().push_back(s)))
                {
                    log::error!("Error on message: {}", e);
                    self.close_socket(Some(CloseFrame { code:   CloseCode::Error,
                                                        reason: e.to_string().into() }));
                    return false;
                }
                while let Some(msg) = q.lock().unwrap().pop_front()
                {
                    if let Err(e) = self.socket.write_message(Message::Text(msg))
                    {
                        log::error!("Error writing message back to client: {e}");
                        self.close_socket(Some(CloseFrame { code:   CloseCode::Error,
                                                            reason: e.to_string().into() }));
                        return false;
                    }
                }
            }
            Message::Binary(_) =>
            {
                unimplemented!("Binary data not supported")
            }
            Message::Ping(_) =>
            {
                log::debug!("Receiving Ping.")
            }
            Message::Pong(_) =>
            {
                log::debug!("Receiving Pong.")
            }
            Message::Close(c) =>
            {
                self.close_socket(c);
                return false;
            }
            Message::Frame(_) =>
            {
                unreachable!()
            }
        }
        true
    }

    fn close_socket(&mut self, cf: Option<CloseFrame>)
    {
        let _ = self.socket.close(cf);
        let timeout = Instant::now() + Duration::from_secs(10);
        while self.socket.write_pending().is_ok() && timeout < Instant::now()
        {
            std::thread::yield_now()
        }
    }
}

impl SockleServer for SimpleSockleServer
{
    fn listen<F: Fn(String, Box<dyn Fn(String)>) -> Result<()> + Send + Sync + 'static>(
        &mut self,
        listen_address: &str,
        on_message: F)
        -> Result<()>
    {
        let server = TcpListener::bind(listen_address)?;
        server.set_nonblocking(true)?;
        let on_message: OnMessageFn = Arc::new(on_message);
        let senders = self.thread_senders.clone();
        let (thread_ctrl_s, thread_ctrl_r) = std::sync::mpsc::channel();
        self.thread_ctrl = Some(thread_ctrl_s);
        std::thread::Builder::new().name("Sockle Server Connection Listener".to_string()).spawn(move || {
            for stream in server.incoming()
            {
                match stream
                {
                    Ok(s) =>
                    {
                        let on_message_t = on_message.clone();
                        let senders2 = senders.clone();
                        std::thread::Builder::new().name("Sockle Server Client Connection".to_string()).spawn(move || {
                            match tungstenite::accept(s)
                            {
                                Ok(socket) =>
                                {
                                    let r = {
                                        let mut s = senders2.lock().unwrap();
                                        let (sender, r) = std::sync::mpsc::channel();
                                        s.push(sender);
                                        r
                                    };
                                    Conn::new(socket, r, on_message_t).on_accept();
                                }
                                Err(e) =>
                                {
                                    log::error!("Error accepting incoming stream: {e}");
                                }
                            }
                        }).unwrap();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        std::thread::sleep(Duration::from_millis(15));
                    }
                    Err(e) =>
                    {
                        log::error!("Error opening incoming stream: {e}");
                    }
                }

                if matches!(thread_ctrl_r.try_recv(), Err(TryRecvError::Disconnected) | Ok(_))
                {
                    log::debug!("Server shutdown requested, ending listen thread");
                    break;
                }

            }
            log::info!("Sockle server has shutdown");
        })?;
        Ok(())
    }

    fn send(&self, msg: String)
    {
        for s in self.thread_senders.lock().unwrap().iter()
        {
            let _ = s.send(SockleServerMessage::Send(msg.clone()));
        }
    }

    fn shutdown(&self) -> Result<()>
    {
        for s in self.thread_senders.lock().unwrap().iter()
        {
            let _ = s.send(SockleServerMessage::Shutdown);
        }
        let tc = self.thread_ctrl.as_ref().unwrap();
        if let Err(e) = tc.send(())
        {
            let err = format!("Unable to signal listen thread to end: {e}");
            log::error!("{err}");
            anyhow::bail!(err);
        }
        while let Ok(_) = tc.send(())
        {
            std::thread::yield_now();
        }
        Ok(())
    }

    fn connection_count(&self) -> usize
    {
        self.thread_senders.lock().unwrap().len()
    }
}
