use async_std::net::TcpStream;
use async_tungstenite::tungstenite;
use async_tungstenite::tungstenite::protocol::Message;
use async_tungstenite::WebSocketStream;
use futures::stream::SplitSink;
use protocol::ServerMessage;
use thiserror::Error;

#[derive(Debug)]
pub struct SocketSender(SplitSink<WebSocketStream<TcpStream>, Message>);

impl SocketSender {
    pub fn new(sender: SplitSink<WebSocketStream<TcpStream>, Message>) -> Self {
        Self(sender)
    }

    pub async fn send(&mut self, message: ServerMessage) -> Result<(), SocketMessageSendError> {
        use bincode::serialize;
        use futures::SinkExt;

        let message: Vec<u8> = serialize(&message)?;
        self.0.send(Message::Binary(message)).await?;
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum SocketMessageSendError {
    #[error("message serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
    #[error("WebSocket send message error: {0}")]
    WebSocketSendError(#[from] tungstenite::Error),
}
