use std::net::SocketAddr;
use std::sync::Arc;

use async_std::net::TcpStream;
use async_std::sync::Mutex;
use protocol::{PeerId, ServerMessage};
use thiserror::Error;

use crate::{
    SocketMessageReceiveError, SocketMessageSendError, SocketReceiver, SocketSender, State,
};

#[derive(Debug)]
pub struct Socket {
    sender: Arc<Mutex<SocketSender>>,
    receiver: SocketReceiver,
    addr: SocketAddr,
    state: Arc<State>,
}

impl Socket {
    pub async fn new(stream: TcpStream, addr: SocketAddr, state: Arc<State>) -> Self {
        use async_tungstenite::accept_async;
        use futures::StreamExt;

        let stream = accept_async(stream).await.unwrap();
        let (sender, receiver) = stream.split();
        let sender = Arc::new(Mutex::new(SocketSender::new(sender)));
        let receiver = SocketReceiver::new(receiver);

        Self {
            sender,
            receiver,
            addr,
            state,
        }
    }

    pub async fn run(mut self) -> Result<(), SocketRunError> {
        use protocol::ClientMessage;

        let addr = self.addr;
        log::info!("socket {} opened", addr);

        let peer_id = self.state.new_peer(&self.sender).await;
        log::info!("socket {} client id assigned", peer_id);
        self.sender
            .lock()
            .await
            .send(ServerMessage::PeerIdAssigned { peer_id })
            .await?;

        while let Some(message) = self.receiver.recv().await? {
            match message {
                ClientMessage::RequestOffers { file_sha256 } => {
                    let peer_list = self
                        .state
                        .add_file_peer_and_get_file_peer_list(file_sha256, peer_id)
                        .await
                        .unwrap();

                    for other_peer_id in peer_list {
                        if peer_id == other_peer_id {
                            continue;
                        }

                        self.send_to_peer(
                            other_peer_id,
                            ServerMessage::OfferRequest {
                                peer_id: other_peer_id,
                            },
                        )
                        .await?;
                    }
                }
                ClientMessage::SendOffer { peer_id, offer } => {
                    self.send_to_peer(peer_id, ServerMessage::PeerOffer { peer_id, offer })
                        .await?;
                }
                ClientMessage::SendAnswer { peer_id, answer } => {
                    self.send_to_peer(peer_id, ServerMessage::PeerAnswer { peer_id, answer })
                        .await?;
                }
                ClientMessage::SendIceCandidate { peer_id, candidate } => {
                    self.send_to_peer(
                        peer_id,
                        ServerMessage::PeerIceCandidate { peer_id, candidate },
                    )
                    .await?;
                }
                ClientMessage::AllIceCandidatesSent { peer_id } => {
                    self.send_to_peer(peer_id, ServerMessage::PeerAllIceCandidatesSent { peer_id })
                        .await?;
                }
            }
        }

        log::info!("socket {} closed", addr);
        Ok(())
    }

    async fn send_to_peer(
        &self,
        peer_id: PeerId,
        message: ServerMessage,
    ) -> Result<(), SocketMessageSendError> {
        let sender = self.state.get_peer_sender(peer_id).await;
        if let Some(sender) = sender {
            sender.lock().await.send(message).await?;
        }
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum SocketRunError {
    #[error(transparent)]
    MessageReceiveError(#[from] SocketMessageReceiveError),
    #[error(transparent)]
    MessageSendError(#[from] SocketMessageSendError),
}
