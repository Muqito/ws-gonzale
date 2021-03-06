use {
    crate::{AsyncResult, WsGonzaleError, WsGonzaleResult},
    async_net::TcpStream,
    base64::encode,
    futures::AsyncReadExt,
    futures::AsyncWriteExt,
    sha1::Sha1,
    std::collections::HashMap,
    std::ops::Deref,
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
/// HTTP Request Headers
#[derive(Debug)]
pub struct Headers(HashMap<String, String>);

impl Headers {
    pub fn new(headers: HashMap<String, String>) -> Self {
        Self(headers)
    }
    pub fn get(&self, key: &str) -> Option<&String> {
        self.0.get(key)
    }
}
/// Quickly writes a response to the TcpStream with a valid `Sec-Websocket-Accept: {key}` if available
pub async fn handshake(key: &str, tcp_stream: &mut TcpStream) -> AsyncResult<()> {
    let accept_key = get_accept_from_key(&key).unwrap_or("".to_string());
    // Just a quick reply with the `Sec-Websocket-Accept: {key}`
    let returned_string = format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-Websocket-Accept: {accept_key}\r\n\r\n", accept_key = accept_key);
    tcp_stream.write_all(returned_string.as_bytes()).await?; // Accept the connection
    Ok(())
}

/// HTTP Methods
#[derive(Debug, PartialEq)]
pub enum HTTPMethod {
    GET,
    POST,
    DELETE,
    Unknown,
}
/// HTTP Request URI
#[derive(Debug)]
pub struct Uri(String);
impl Deref for Uri {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}
/// HTTP Request Endpoints
#[derive(Debug)]
pub struct Endpoint {
    method: HTTPMethod,
    uri: Uri,
}
impl Endpoint {
    pub fn new(s: &str) -> Endpoint {
        let mut splits = s.split(" ");
        Endpoint {
            method: match splits.next().unwrap_or("") {
                "GET" => HTTPMethod::GET,
                "POST" => HTTPMethod::POST,
                _ => HTTPMethod::Unknown,
            },
            uri: Uri(splits
                .next()
                .map(|s| s.to_string())
                .unwrap_or("".to_string())),
        }
    }
    pub fn get_method(&self) -> &HTTPMethod {
        &self.method
    }
    pub fn get_uri(&self) -> &Uri {
        &self.uri
    }
}
/// HTTP Request Body
#[derive(Debug, PartialEq)]
pub struct Body(String);
impl Body {
    pub fn get_body(&self) -> &str {
        &self.0
    }
}

/// HTTP Request
#[derive(Debug)]
pub struct Request {
    endpoint: Endpoint,
    headers: Headers,
    body: Option<Body>,
}
impl Request {
    pub fn from_str(s: &str) -> WsGonzaleResult<Request> {
        let mut request = s.lines().collect::<Vec<&str>>();
        let empty_index = request.iter().position(|s| s.is_empty());

        let mut body: Option<Body> = None;
        if let Some(empty_index) = empty_index {
            body = Some(Body(request.split_off(empty_index).join("")));
        }

        let mut iters = request.iter();
        let endpoint = iters.next();
        if endpoint.is_none() {
            return Err(WsGonzaleError::InvalidPayload);
        }
        let endpoint = Endpoint::new(endpoint.unwrap());

        let headers = {
            let headers: HashMap<String, String> = iters
                .flat_map(|val| {
                    let mut splits = val.splitn(2, ": ");
                    match (splits.next(), splits.next()) {
                        (Some(key), Some(value)) => Some((key.to_string(), value.to_string())),
                        _ => None,
                    }
                })
                .collect();

            Headers::new(headers)
        };

        let data = match (&endpoint.method, body) {
            (HTTPMethod::POST, body @ Some(_)) => Request {
                endpoint,
                headers,
                body,
            },
            _ => Request {
                endpoint,
                headers,
                body: None,
            },
        };
        Ok(data)
    }
    pub async fn read_from_stream(tcp_stream: &mut TcpStream) -> WsGonzaleResult<Request> {
        let mut buffers: Vec<u8> = vec![0u8; 100000];
        let number = match tcp_stream.read(&mut buffers).await {
            Ok(n) if n == 0 => return Err(WsGonzaleError::Unknown),
            Ok(n) => n,
            Err(_err) => return Err(WsGonzaleError::Unknown),
        };
        let buffers: Vec<u8> = buffers.into_iter().take(number).collect();
        let s = String::from_utf8_lossy(&buffers).to_string();
        Request::from_str(&s)
    }
    pub fn get_endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
    pub fn get_headers(&self) -> &Headers {
        &self.headers
    }
    pub fn get_body(&self) -> Option<&Body> {
        self.body.as_ref()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_small_post() {
        let request = r#"POST / HTTP/1.1
        Content-Type: application/javascript
        User-Agent: PostmanRuntime/7.26.1
        Accept: */*
        Cache-Control: no-cache
        Postman-Token: dc343e4f-6cf3-4d6d-827e-7fbbd82d80fb
        Host: 127.0.0.1:8080
        Accept-Encoding: gzip, deflate, br
        Connection: keep-alive
        Content-Length: 15

        {
            id: 5
        }"#;
        let result = Request::from_str(request).unwrap();
        let mut body = result.body.unwrap().0;
        body.retain(|c| !c.is_whitespace());
        assert_eq!(result.endpoint.method, HTTPMethod::POST);
        assert_eq!(body.len(), 6);
    }
    #[test]
    fn test_post_without_data() {
        let request = r#"POST / HTTP/1.1
        Content-Type: application/javascript
        User-Agent: PostmanRuntime/7.26.1
        Accept: */*
        Cache-Control: no-cache
        Postman-Token: dc343e4f-6cf3-4d6d-827e-7fbbd82d80fb
        Host: 127.0.0.1:8080
        Accept-Encoding: gzip, deflate, br
        Connection: keep-alive
        Content-Length: 15
"#;
        let result = Request::from_str(request).unwrap();
        assert_eq!(result.endpoint.method, HTTPMethod::POST);
        assert_eq!(result.body, None);
    }
    #[test]
    fn test_post_without_any_data() {
        let request = "";
        assert_eq!(
            Request::from_str(request).err().unwrap(),
            WsGonzaleError::InvalidPayload
        );
    }
    #[test]
    fn test_post_without_headers_ending() {
        let request = r#"POST / HTTP/1.1
        Content-Type: application/javascript
        User-Agent: PostmanRuntime/7.26.1
        Accept: */*
        Cache-Control: no-cache
        Postman-Token: dc343e4f-6cf3-4d6d-827e-7fbbd82d80fb
        Host: 127.0.0.1:8080
        Accept-Encoding: gzip, deflate, br
        Connection: keep-alive
        Content-Length: 15"#;
        let result = Request::from_str(request).unwrap();
        assert_eq!(result.endpoint.method, HTTPMethod::POST);
        assert_eq!(result.body, None);
    }
    #[test]
    fn test_small_get() {
        let request = r#"GET / HTTP/1.1
        Content-Type: application/javascript
        User-Agent: PostmanRuntime/7.26.1
        Accept: */*
        Cache-Control: no-cache
        Postman-Token: dc343e4f-6cf3-4d6d-827e-7fbbd82d80fb
        Host: 127.0.0.1:8080
        Accept-Encoding: gzip, deflate, br
        Connection: keep-alive
        Content-Length: 15

        {
            id: 5
        }"#;
        let result = Request::from_str(request).unwrap();
        assert_eq!(result.headers.0.len(), 9);
        assert_eq!(result.endpoint.method, HTTPMethod::GET);
        assert_eq!(result.body, None);
    }
}
