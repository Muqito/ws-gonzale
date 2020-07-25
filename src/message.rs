#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Binary(Vec<u8>),
    Text(String),
    Ping(String),
    Pong(String),
    Close,
    Unknown,
}
