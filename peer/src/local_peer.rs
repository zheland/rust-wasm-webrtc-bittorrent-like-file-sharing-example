use core::cell::RefCell;
use core::time::Duration;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

use async_std::sync::RwLock;
use thiserror::Error;
use tracker_protocol::{FileSha256, PeerId, PeerTrackerMessage, TrackerPeerMessage};

use crate::{
    FileGetPieceError, FilePieceIdx, FileSentFileStateError, LocalFile, NowError, PeerPeerMessage,
    RemotePeer, Tracker,
};

#[derive(Debug)]
pub struct LocalPeer {
    tracker: Tracker,
    peer_id: RefCell<Option<PeerId>>,
    peers: RwLock<HashMap<PeerId, Arc<RemotePeer>>>,
    files: RwLock<HashMap<FileSha256, Weak<LocalFile>>>,
}

impl LocalPeer {
    pub async fn new(tracker_addr: String) -> Arc<Self> {
        let peer = Arc::new(LocalPeer {
            tracker: Tracker::new(tracker_addr).await,
            peer_id: RefCell::new(None),
            peers: RwLock::new(HashMap::new()),
            files: RwLock::new(HashMap::new()),
        });

        peer.init();

        peer
    }

    fn init(self: &Arc<Self>) {
        use wasm_bindgen_futures::spawn_local;

        let self_weak = Arc::downgrade(self);
        self.tracker.set_handler(move |msg| {
            if let Some(self_arc) = self_weak.upgrade() {
                spawn_local(async move { self_arc.on_tracker_message(msg).await });
            }
        });
    }

    pub fn files(&self) -> &RwLock<HashMap<FileSha256, Weak<LocalFile>>> {
        &self.files
    }

    pub fn send(&self, message: PeerTrackerMessage) {
        log::trace!("send tracker_message {:?}", message); // TODO: Remove
        self.tracker.send(message);
    }

    async fn on_tracker_message(self: &Arc<Self>, message: TrackerPeerMessage) {
        use crate::{unwrap_or_return, RemotePeerKind};
        use std::collections::hash_map::Entry;

        log::trace!("recv tracker_message {:?}", message); // TODO: Remove
        match message {
            TrackerPeerMessage::PeerIdAssigned { peer_id } => {
                let prev_id: Option<_> = self.peer_id.replace(Some(peer_id));
                assert_eq!(prev_id, None);
            }
            TrackerPeerMessage::RequestOffer {
                peer_id,
                file_sha256,
            } => {
                let local_file = self
                    .files
                    .read()
                    .await
                    .get(&file_sha256)
                    .and_then(|file| file.upgrade());
                let local_file = unwrap_or_return!(local_file);

                let mut peers = self.peers.write().await;

                let remote_peer = peers.entry(peer_id);
                match remote_peer {
                    Entry::Occupied(_) => {}
                    Entry::Vacant(entry) => {
                        let remote_peer =
                            RemotePeer::new(self, peer_id, RemotePeerKind::Offering).await;
                        let _: &mut _ = entry.insert(remote_peer);
                    }
                };
                local_file.add_remote(peer_id).await;
            }
            TrackerPeerMessage::PeerOffer { peer_id, offer } => {
                let mut peers = self.peers.write().await;
                let remote_peer = peers.entry(peer_id);
                let remote_peer = match remote_peer {
                    Entry::Occupied(entry) => Arc::clone(entry.get()),
                    Entry::Vacant(entry) => {
                        let remote_peer =
                            RemotePeer::new(self, peer_id, RemotePeerKind::Answering).await;
                        let _: &mut _ = entry.insert(Arc::clone(&remote_peer));
                        remote_peer
                    }
                };
                remote_peer.on_peer_offer(offer).await;
            }
            TrackerPeerMessage::PeerAnswer { peer_id, answer } => {
                let peers = self.peers.read().await;
                if let Some(remote_peer) = peers.get(&peer_id) {
                    remote_peer.on_peer_answer(answer).await;
                } else {
                    log::error!("unexpected answer from peer {}", peer_id);
                };
            }
            TrackerPeerMessage::PeerIceCandidate { peer_id, candidate } => {
                let peers = self.peers.read().await;
                if let Some(remote_peer) = peers.get(&peer_id) {
                    remote_peer.on_peer_icecandidate(candidate).await;
                } else {
                    log::error!("unexpected icecandidate from peer {}", peer_id);
                };
            }
            TrackerPeerMessage::PeerAllIceCandidatesSent { peer_id } => {
                let peers = self.peers.read().await;
                if let Some(remote_peer) = peers.get(&peer_id) {
                    remote_peer.on_peer_all_icecandidates_sent().await;
                } else {
                    log::error!("unexpected all_icecandidates_sent from peer {}", peer_id);
                };
            }
        }
    }

    pub async fn on_peer_message(
        self: &Arc<Self>,
        remote_peer: &Arc<RemotePeer>,
        message: PeerPeerMessage,
    ) {
        use crate::{unwrap_or_return, FileState};

        let peer_id = remote_peer.peer_id();

        let sha256 = *match &message {
            PeerPeerMessage::FileMissing { sha256 } => sha256,
            PeerPeerMessage::FileComplete { sha256 } => sha256,
            PeerPeerMessage::FileState { sha256, state: _ } => sha256,
            PeerPeerMessage::FileStateReceived { sha256 } => sha256,
            PeerPeerMessage::FilePiece {
                sha256,
                piece_idx: _,
                bytes: _,
            } => sha256,
            PeerPeerMessage::FilePiecesReceived { sha256, pieces: _ } => sha256,
            PeerPeerMessage::FileRemoved { sha256 } => sha256,
        };

        let file = unwrap_or_return!(self.get_file(sha256).await);
        file.add_remote(peer_id).await;
        match &message {
            PeerPeerMessage::FilePiece {
                sha256,
                piece_idx,
                bytes: _,
            } => {
                #[derive(Clone, Debug)]
                struct FilePiece<'a> {
                    sha256: &'a FileSha256,
                    piece_idx: &'a FilePieceIdx,
                }
                log::trace!(
                    "recv peer_message {} {:?}",
                    peer_id,
                    FilePiece { sha256, piece_idx }
                ); // TODO: Remove
            }
            message => {
                log::trace!("recv peer_message {} {:?}", peer_id, message); // TODO: Remove
            }
        }

        match message {
            PeerPeerMessage::FileMissing { sha256: _ } => {
                file.set_remote_file_missing(peer_id).await;
            }
            PeerPeerMessage::FileComplete { sha256: _ } => {
                file.set_remote_file_complete(peer_id).await;
            }
            PeerPeerMessage::FileState { sha256: _, state } => {
                file.set_remote_file_state(peer_id, FileState::from(state))
                    .await;
            }
            PeerPeerMessage::FileStateReceived { sha256: _ } => {
                file.on_state_received_by_remote(peer_id).await;
            }
            PeerPeerMessage::FilePiece {
                sha256,
                piece_idx,
                bytes,
            } => {
                file.set_file_piece(piece_idx, bytes).await;
                // TODO: Send a bunch of requests before sending data.
                remote_peer.send(PeerPeerMessage::FilePiecesReceived {
                    sha256,
                    pieces: vec![piece_idx],
                });
            }
            PeerPeerMessage::FilePiecesReceived { sha256: _, pieces } => {
                file.on_remote_pieces_received(peer_id, pieces).await;
            }
            PeerPeerMessage::FileRemoved { sha256: _ } => {
                file.remove_remote_file(peer_id).await;
            }
        }
    }

    pub async fn add_file(&self, file: &Arc<LocalFile>) {
        let mut files = self.files.write().await;
        // TODO: Check overwrite
        let _ = files.insert(file.sha256(), Arc::downgrade(file));
        let message = PeerTrackerMessage::RequestOffers {
            file_sha256: file.metadata().sha256(),
        };
        self.tracker.send(message);
    }

    pub async fn get_file(&self, sha256: FileSha256) -> Option<Arc<LocalFile>> {
        self.files.read().await.get(&sha256).and_then(Weak::upgrade)
    }

    pub async fn clear_removed_files(&self) {
        self.files
            .write()
            .await
            .retain(|_, file| file.strong_count() > 0);
    }

    pub async fn send_state_to_remote_peers(
        &self,
        state_resend_interval: Duration,
    ) -> Result<(), LocalPeerSendStateToRemotePeersError> {
        use crate::{now, RemoteStateStatus};

        let now = now()?;

        let files = self.files.read().await;
        let peers = self.peers.read().await;

        for (_, file) in files.iter() {
            if let Some(file) = file.upgrade() {
                let remotes: Vec<_> = file
                    .remote_status()
                    .await
                    .iter()
                    .filter(|(_, status)| match status {
                        RemoteStateStatus::Received => false,
                        RemoteStateStatus::Sent(timestamp) => {
                            now - *timestamp >= state_resend_interval
                        }
                        RemoteStateStatus::NotSent => true,
                    })
                    .filter_map(|(peer_id, _)| peers.get(peer_id).map(Arc::clone))
                    .collect();
                for peer in remotes {
                    file.send_file_state(peer).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn sent_pieces_to_remote_peers(
        &self,
        num_pieces_to_be_sent: usize,
        max_buffer_bytes: Option<u64>,
    ) -> Result<(), LocalPeerSendStateToRemotePeersError> {
        use crate::{FileSharingSelector, JsRandom, PeerConnectionSendError};
        use rand_chacha::ChaCha8Rng;

        let rng = ChaCha8Rng::new();

        let files = self.files.read().await;
        let peers = self.peers.read().await;

        let files_vec: Vec<_> = files.values().filter_map(Weak::upgrade).collect();
        let mut states = Vec::new();
        for file in &files_vec {
            states.push((file.sha256(), file.sharing_state_mut().await));
        }
        let selector = FileSharingSelector::new(states, rng);
        let selected: Vec<_> = selector.take(num_pieces_to_be_sent).collect();

        for (peer_id, sha256, piece_idx) in selected {
            //log::debug!("{:?} {:?} {:?}", &peer_id, &sha256, piece_idx);
            if let (Some(file), Some(peer)) = (
                files.get(&sha256).and_then(Weak::upgrade),
                peers.get(&peer_id),
            ) {
                let bytes = file
                    .get_piece(piece_idx)
                    .await?
                    .ok_or_else(|| LocalPeerSendStateToRemotePeersError::PieceNotSet {
                        file_piece_idx: piece_idx,
                    })?
                    .into_boxed_slice();
                let message = PeerPeerMessage::FilePiece {
                    sha256,
                    piece_idx,
                    bytes,
                };
                match max_buffer_bytes {
                    Some(max_buffer_bytes) => {
                        match peer.send_with_max_buffer_size(message, max_buffer_bytes) {
                            Ok(()) | Err(PeerConnectionSendError::BufferIsFilled) => {}
                        }
                    }
                    None => peer.send(message),
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Error, Debug)]
pub enum LocalPeerSendStateToRemotePeersError {
    #[error(transparent)]
    NowError(#[from] NowError),
    #[error(transparent)]
    SendError(#[from] FileSentFileStateError),
    #[error(transparent)]
    FileGetPieceError(#[from] FileGetPieceError),
    #[error("file piece {file_piece_idx} is not set")]
    PieceNotSet { file_piece_idx: FilePieceIdx },
}
