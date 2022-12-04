# Sockle

Basic websocket client/server using tungstenite.

### Traits

The simple server/client implement traits `SockleServer`/`SockleClient`.

### TLS support

Client supports TLS, currently no support on the server side.

### Usage

#### Simple echo server
```rust
use Sockle::*;

pub fn main()
{
    let mut server = SimpleSockleServer::new();
    
    // The passed closure is where the incoming message is handled
    // here we just echo the message back to the client
    server.listen("127.0.0.1:8090", |message, responder| {
              responder(message);
              Ok(())
          }).unwrap();
          
    // Thread is now running, do other stuff until ready to shutdown
    // Even if just waiting for ctrl+c
    std::thread::sleep(Duration::from_secs(10));
    
    // When ready shutdown, this will gracefully disconnect clients
    // and end the listening thread 
    server.shutdown();    
}
```

#### Simple client
```rust
pub fn main()
{
    
        let mut client = SimpleSockleClient::new();
        
        // this spawns a dedicated thread to handle the connection
        client.connect("ws://127.0.0.1:8090").unwrap();
        
        // send messages to server
        // pings are automatically responded to with a pong
        client.write("test".to_string()).unwrap();
        
        // Read has 3 flavours, read, try_read and read_timeout
        // read blocks until message is received
        // try_read does not block
        // read_timeout blocks for timeout period
        // the later two return message wrapped in an option
        while let Ok(message) = client.read_timeout(Duration::from_secs(1)).unwrap()
        {
            match messsage
            {
                Some(msg) => println!("Message: {msg}"),
                None => 
                {
                    // No data for over 1 second
                    client.close();
                    return;
                }
            }
        }
}
```