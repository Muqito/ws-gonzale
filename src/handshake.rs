use {
    crate::AsyncResult,
    async_net::TcpStream,
    base64::encode,
    futures::{AsyncReadExt, AsyncWriteExt},
    sha1::Sha1,
    std::collections::HashMap,
    std::string::ToString,
};

const MAGIC_GUID: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

fn sha1_str(s: &str) -> [u8; 20] {
    let mut sha1 = Sha1::new();
    sha1.update(s.as_bytes());
    let bytes = sha1.digest().bytes();
    bytes
}
fn get_accept_from_key(key: &str) -> Result<String, String> {
    let mut accept_key = String::with_capacity(key.len() + 36);
    accept_key.push_str(&key);
    accept_key.push_str(MAGIC_GUID);

    let sha1_accept_key = sha1_str(&accept_key); // sha1 result
    Ok(encode(sha1_accept_key))
}

struct Headers;

impl Headers {
    fn from_buffer(buffers: &[u8]) -> HashMap<String, String> {
        String::from_utf8_lossy(&buffers)
            .split("\r\n")
            .filter(|s| !s.is_empty())
            .flat_map(|val| {
                let mut splits = val.split(": ");
                match (splits.next(), splits.next()) {
                    (Some(key), Some(value)) => Some((key.to_string(), value.to_string())),
                    _ => None,
                }
            })
            .collect()
    }
}

struct Handshake {
    headers: HashMap<String, String>,
}

impl Handshake {
    fn new(headers: HashMap<String, String>) -> Self {
        Self { headers }
    }
    /// Quickly writes a response to the TcpStream with a valid `Sec-Websocket-Accept: {key}` if available
    async fn handshake(&mut self, sender: &mut TcpStream) -> AsyncResult<()> {
        let default_str = String::new();
        let key = self
            .headers
            .get("Sec-WebSocket-Key")
            .unwrap_or(&default_str);
        let accept_key = get_accept_from_key(&key).unwrap_or("".to_string());
        // Just a quick reply with the `Sec-Websocket-Accept: {key}`
        let returned_string = format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-Websocket-Accept: {accept_key}\r\n\r\n", accept_key = accept_key);
        sender.write_all(returned_string.as_bytes()).await?; // Accept the connection
        Ok(())
    }
}
/// Upgrades the incoming GET request to a keep-open WebSocket connection
pub async fn handshake(mut stream: &mut TcpStream) -> AsyncResult<()> {
    let mut buffers: Vec<u8> = vec![0u8; 1000];
    stream.read(&mut buffers).await?;
    let headers: HashMap<String, String> = Headers::from_buffer(&buffers);
    let mut upgrade_header = Handshake::new(headers);
    upgrade_header.handshake(&mut stream).await?;
    Ok(())
}
