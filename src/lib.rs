mod client;
pub use client::*;

mod server;
pub use server::*;

mod error;
pub use error::SimpleSockleError;

#[cfg(test)]
mod tests
{
    use super::*;
    use std::time::Duration;

    #[test]
    fn when_no_data_try_read_should_return_none()
    {
        let _ = pretty_env_logger::try_init();
        let mut s = SimpleSockleClient::new();
        let mut server = SimpleSockleServer::new();
        server.listen("127.0.0.1:8924", |_, _| Ok(())).unwrap();

        s.connect("ws://127.0.0.1:8924/").expect("Connect");
        assert!(s.try_read().unwrap().is_none());

        server.shutdown().unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }

    #[test]
    fn when_no_data_read_timeout_should_return_none()
    {
        let _ = pretty_env_logger::try_init();
        let mut s = SimpleSockleClient::new();
        let mut server = SimpleSockleServer::new();
        server.listen("127.0.0.1:8925", |_, _| Ok(())).unwrap();

        s.connect("ws://127.0.0.1:8925/").expect("Connect");
        assert!(s.read_timeout(Duration::from_millis(15)).unwrap().is_none());

        server.shutdown().unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }
}
