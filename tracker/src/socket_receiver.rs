use async_std::net::TcpStream;
use async_tungstenite::tungstenite;
use async_tungstenite::tungstenite::protocol::Message;
use async_tungstenite::WebSocketStream;
use futures::stream::SplitStream;
use thiserror::Error;
use tracker_protocol::PeerTrackerMessage;

#[derive(Debug)]
pub struct SocketReceiver(SplitStream<WebSocketStream<TcpStream>>);

#[allow(single_use_lifetimes)] // false positive
impl SocketReceiver {
    pub fn new(receiver: SplitStream<WebSocketStream<TcpStream>>) -> Self {
        Self(receiver)
    }

    pub async fn recv(&mut self) -> Result<Option<PeerTrackerMessage>, SocketMessageReceiveError> {
        use bincode::deserialize;
        use futures::StreamExt;

        let message = self
            .0
            .next()
            .await
            .ok_or(SocketMessageReceiveError::UnexpectedEndOfStream)??;
        match message {
            Message::Binary(data) => Ok(Some(deserialize(&data[..])?)),
            Message::Close(_) => Ok(None),
            message => Err(SocketMessageReceiveError::InvalidWebSocketMessage(message)),
        }
    }
}

#[derive(Error, Debug)]
pub enum SocketMessageReceiveError {
    #[error("unexpectedEndOfStream")]
    UnexpectedEndOfStream,
    #[error("message deserialization error: {0}")]
    DeserializationError(#[from] bincode::Error),
    #[error("WebSocket receive message error: {0}")]
    WebSocketReceiveError(#[from] tungstenite::Error),
    #[error("invalid WebSocket message: {0}")]
    InvalidWebSocketMessage(Message),
}
