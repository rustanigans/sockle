mod client;
pub use client::*;

mod server;
pub use server::{SimpleSockleServer, SockleServer};

mod error;
pub use error::SimpleSockleError;

#[cfg(test)]
mod tests
{
    use super::*;
    use std::{sync::atomic::{AtomicUsize, Ordering},
              time::Duration};

    fn listen_addr() -> (String, String)
    {
        static PORT: AtomicUsize = AtomicUsize::new(8469);
        let port = PORT.fetch_add(1, Ordering::SeqCst);
        (format!("127.0.0.1:{}", port), format!("ws://127.0.0.1:{}/", port))
    }

    fn wait_for_connections(server: &SimpleSockleServer, count: usize)
    {
        while server.connection_count() < count
        {
            std::thread::yield_now()
        }
    }

    #[test]
    fn when_no_data_try_read_should_return_none()
    {
        let _ = pretty_env_logger::try_init();
        let mut s = SimpleSockleClient::new();
        let mut server = SimpleSockleServer::new();
        let addr = listen_addr();
        server.listen(&addr.0, |_, _| Ok(())).unwrap();

        s.connect(&addr.1).expect("Connect");

        assert!(s.try_read().unwrap().is_none());

        server.shutdown().unwrap();
    }

    #[test]
    fn when_no_data_read_timeout_should_return_none()
    {
        let _ = pretty_env_logger::try_init();
        let mut s = SimpleSockleClient::new();
        let mut server = SimpleSockleServer::new();
        let addr = listen_addr();
        server.listen(&addr.0, |_, _| Ok(())).unwrap();

        s.connect(&addr.1).expect("Connect");

        assert!(s.read_timeout(Duration::from_millis(15)).unwrap().is_none());

        server.shutdown().unwrap();
    }

    #[test]
    fn when_data_read_should_return_data()
    {
        let _ = pretty_env_logger::try_init();
        let mut s = SimpleSockleClient::new();
        let mut server = SimpleSockleServer::new();
        let addr = listen_addr();
        server.listen(&addr.0, |_, _| Ok(())).unwrap();

        s.connect(&addr.1).expect("Connect");

        wait_for_connections(&server, 1);

        server.send("Test".to_string());

        assert_eq!(s.read().unwrap(), "Test");

        server.shutdown().unwrap();
    }

    #[test]
    fn echo_server()
    {
        let _ = pretty_env_logger::try_init();
        let mut s = SimpleSockleClient::new();
        let mut server = SimpleSockleServer::new();
        let addr = listen_addr();
        server.listen(&addr.0, |m, f| {
                  f(m);
                  Ok(())
              })
              .unwrap();

        s.connect(&addr.1).unwrap();

        wait_for_connections(&server, 1);

        s.write("Test".to_string()).unwrap();

        assert_eq!(s.read().unwrap(), "Test");

        server.shutdown().unwrap();
    }

    #[test]
    fn broadcast()
    {
        let _ = pretty_env_logger::try_init();
        let mut s1 = SimpleSockleClient::new();
        let mut s2 = SimpleSockleClient::new();
        let mut server = SimpleSockleServer::new();
        let addr = listen_addr();
        server.listen(&addr.0, |_, _| Ok(())).unwrap();

        s1.connect(&addr.1).unwrap();
        s2.connect(&addr.1).unwrap();

        wait_for_connections(&server, 2);

        server.send("Test".to_string());

        assert_eq!(s1.read().unwrap(), "Test");
        assert_eq!(s2.read().unwrap(), "Test");

        server.shutdown().unwrap();
    }
}
