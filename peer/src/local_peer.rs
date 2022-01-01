use core::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

use async_std::sync::RwLock;
use thiserror::Error;
use tracker_protocol::{FileSha256, PeerId, PeerTrackerMessage, TrackerPeerMessage};

use crate::{JsFile, JsSharedFile, PeerPeerMessage, RemotePeer, Tracker};

#[derive(Debug)]
pub struct LocalPeer<T> {
    tracker: Tracker,
    peer_id: RefCell<Option<PeerId>>,
    peers: RwLock<HashMap<PeerId, Arc<RemotePeer<T>>>>,
    files: RwLock<HashMap<FileSha256, Weak<RwLock<JsSharedFile<T>>>>>,
}

impl<T> LocalPeer<T> {
    pub async fn new(tracker_addr: String) -> Arc<Self>
    where
        T: 'static + Ord,
    {
        let peer = Arc::new(LocalPeer {
            tracker: Tracker::new(tracker_addr).await,
            peer_id: RefCell::new(None),
            peers: RwLock::new(HashMap::new()),
            files: RwLock::new(HashMap::new()),
        });

        peer.init();

        peer
    }

    fn init(self: &Arc<Self>)
    where
        T: 'static + Ord,
    {
        use wasm_bindgen_futures::spawn_local;

        let self_weak = Arc::downgrade(self);
        self.tracker.set_handler(move |msg| {
            if let Some(self_arc) = self_weak.upgrade() {
                spawn_local(async move { self_arc.on_tracker_message(msg).await });
            }
        });
    }

    pub fn files(&self) -> &RwLock<HashMap<FileSha256, Weak<RwLock<JsSharedFile<T>>>>> {
        &self.files
    }

    pub fn send(&self, message: PeerTrackerMessage) {
        log::trace!("send tracker_message {:?}", message);
        self.tracker.send(message);
    }

    async fn on_tracker_message(self: &Arc<Self>, message: TrackerPeerMessage)
    where
        T: 'static + Ord,
    {
        use crate::{unwrap_or_return, IgnoreEmpty, OkOrLog, RemotePeerKind};
        use std::collections::hash_map::Entry;

        log::trace!("recv tracker_message {:?}", message);
        match message {
            TrackerPeerMessage::PeerIdAssigned { peer_id } => {
                let prev_id: Option<_> = self.peer_id.replace(Some(peer_id));
                assert_eq!(prev_id, None);
            }
            TrackerPeerMessage::RequestOffer {
                peer_id,
                file_sha256,
            } => {
                let shared_file = self
                    .files
                    .read()
                    .await
                    .get(&file_sha256)
                    .and_then(|file| file.upgrade());
                let shared_file = unwrap_or_return!(shared_file);

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
                shared_file
                    .write()
                    .await
                    .add_peer(peer_id)
                    .ok_or_log()
                    .ignore_empty();
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
        remote_peer: &Arc<RemotePeer<T>>,
        message: PeerPeerMessage,
    ) where
        T: Ord,
    {
        use crate::{
            unwrap_or_return, FileState, IgnoreEmpty, OkOrLog, SharedFileAddPeerError,
            SharedFileLocalStateStatus, SharedFileMarkStatus,
        };

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

        let shared_file = unwrap_or_return!(self.get_file(sha256).await);
        let mut shared_file = shared_file.write().await;
        match shared_file.add_peer(peer_id) {
            Ok(()) | Err(SharedFileAddPeerError::PeerIsAlreadyAdded) => {}
        };

        match message {
            PeerPeerMessage::FileMissing { sha256 } => {
                shared_file
                    .set_peer_file_missing(peer_id)
                    .ok_or_log()
                    .ignore_empty();
                remote_peer.send(PeerPeerMessage::FileStateReceived { sha256 });
            }
            PeerPeerMessage::FileComplete { sha256 } => {
                shared_file
                    .set_peer_file_complete(peer_id)
                    .ok_or_log()
                    .ignore_empty();
                remote_peer.send(PeerPeerMessage::FileStateReceived { sha256 });
            }
            PeerPeerMessage::FileState { sha256, state } => {
                shared_file
                    .set_peer_state(peer_id, FileState::from(state))
                    .ok_or_log()
                    .ignore_empty();
                remote_peer.send(PeerPeerMessage::FileStateReceived { sha256 });
            }
            PeerPeerMessage::FileStateReceived { sha256: _ } => {
                shared_file
                    .local_state_status_mut(&peer_id)
                    .ok_or_log()
                    .map(|status| *status = SharedFileLocalStateStatus::Received)
                    .ignore_empty();
            }
            PeerPeerMessage::FilePiece {
                sha256: _,
                piece_idx,
                bytes,
            } => {
                if !shared_file
                    .file()
                    .has_piece(&piece_idx)
                    .ok_or_log()
                    .unwrap_or(false)
                {
                    shared_file
                        .add_local_piece(piece_idx, &bytes)
                        .ok_or_log()
                        .ignore_empty();
                }
            }
            PeerPeerMessage::FilePiecesReceived { sha256: _, pieces } => {
                for piece in pieces {
                    let _: Option<SharedFileMarkStatus> = shared_file
                        .mark_peer_piece_as_received_by_remote(&peer_id, piece)
                        .ok_or_log();
                }
            }
            PeerPeerMessage::FileRemoved { sha256: _ } => {
                shared_file.remove_peer(&peer_id).ok_or_log().ignore_empty();
            }
        }
    }

    pub async fn add_file(
        &self,
        file: JsFile,
    ) -> Result<Arc<RwLock<JsSharedFile<T>>>, LocalPeerAddFileError> {
        use std::collections::hash_map::Entry;

        let mut files = self.files.write().await;

        let entry = files.entry(file.sha256());
        match entry {
            Entry::Vacant(entry) => {
                let shared_file = Arc::new(RwLock::new(JsSharedFile::new(file)));
                let file_sha256 = *entry.key();
                let _: &mut _ = entry.insert(Arc::downgrade(&shared_file));
                let message = PeerTrackerMessage::RequestOffers { file_sha256 };
                self.tracker.send(message);
                Ok(shared_file)
            }
            Entry::Occupied(_) => Err(LocalPeerAddFileError::AlreadyAdded),
        }
    }

    pub async fn get_file(&self, sha256: FileSha256) -> Option<Arc<RwLock<JsSharedFile<T>>>> {
        self.files.read().await.get(&sha256).and_then(Weak::upgrade)
    }

    pub async fn clear_removed_files(&self) {
        self.files
            .write()
            .await
            .retain(|_, file| file.strong_count() > 0);
    }

    pub async fn send_state_to_remote_peers(&self, resend_before: T, current_time: T)
    where
        T: Clone + PartialOrd,
    {
        use crate::{LocalStateStatusError, SharedFileLocalStateStatus};

        let files = self.files.read().await;
        let peers = self.peers.read().await;

        for (sha256, file) in files.iter() {
            if let Some(shared_file) = file.upgrade() {
                let mut shared_file = shared_file.write().await;
                let peer_ids: Vec<_> = shared_file.peer_ids().copied().collect();
                for peer_id in peer_ids {
                    let local_state_status = match shared_file.local_state_status_mut(&peer_id) {
                        Ok(status) => status,
                        Err(LocalStateStatusError::PeerIsNotAdded) => unreachable!(),
                    };
                    let should_resend = match local_state_status {
                        SharedFileLocalStateStatus::NotSent => true,
                        SharedFileLocalStateStatus::Sent(time) => *time <= resend_before,
                        SharedFileLocalStateStatus::Received => false,
                    };
                    if should_resend {
                        let remote_peer = peers.get(&peer_id).unwrap();

                        if remote_peer.is_ready() {
                            *local_state_status =
                                SharedFileLocalStateStatus::Sent(current_time.clone());

                            let state = shared_file.file().state();

                            if state.is_missing() {
                                remote_peer.send(PeerPeerMessage::FileMissing { sha256: *sha256 });
                            } else if state.is_complete() {
                                remote_peer.send(PeerPeerMessage::FileComplete { sha256: *sha256 });
                            } else {
                                remote_peer.send(PeerPeerMessage::FileState {
                                    sha256: *sha256,
                                    state: state.raw().to_bitvec().into_boxed_bitslice(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn send_recently_received_to_remote_peers(&self) {
        let files = self.files.read().await;
        let peers = self.peers.read().await;

        for (sha256, file) in files.iter() {
            if let Some(shared_file) = file.upgrade() {
                let mut shared_file = shared_file.write().await;
                let pieces = shared_file.take_recently_added_pieces();
                if !pieces.is_empty() {
                    for peer_id in shared_file.peer_ids() {
                        let remote_peer = peers.get(&peer_id).unwrap();
                        if remote_peer.is_ready() {
                            remote_peer.send(PeerPeerMessage::FilePiecesReceived {
                                sha256: *sha256,
                                pieces: pieces.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    pub async fn resend_pieces_before(&self, time: T)
    where
        T: Clone + Ord,
    {
        use crate::ok_or_log::OrLog;

        let files = self.files.read().await;
        let files = files.values().filter_map(Weak::upgrade);

        for file in files {
            file.write()
                .await
                .mark_pieces_for_resend_before(time.clone())
                .or_log();
        }
    }

    pub async fn send_pieces_to_remote_peers(
        &self,
        mut num_pieces_to_be_sent: usize,
        max_buffer_bytes: Option<u64>,
        current_time: T,
        mut rng: impl rand::Rng,
    ) where
        T: Clone + Ord,
    {
        use crate::{PeerConnectionSendError, PieceNumPossibleOwners};
        use core::cmp::Ordering;

        let files: Vec<_> = self
            .files
            .read()
            .await
            .values()
            .filter_map(Weak::upgrade)
            .collect();
        let peers = self.peers.read().await;

        while num_pieces_to_be_sent > 0 {
            let mut min_possible_owners = None;
            let mut file_pieces = Vec::new();

            for (file_idx, shared_file) in files.iter().enumerate() {
                let shared_file = shared_file.read().await;
                let piece_queues = shared_file.piece_queues();
                let queue = piece_queues.next_queue();

                if let Some((file_min_possible_owners, pieces)) = queue {
                    match file_min_possible_owners
                        .cmp(&min_possible_owners.unwrap_or(PieceNumPossibleOwners(usize::MAX)))
                    {
                        Ordering::Less => {
                            min_possible_owners = Some(file_min_possible_owners);
                            if file_min_possible_owners < shared_file.num_peers_with_state() {
                                file_pieces
                                    .extend(pieces.iter().map(|&piece_idx| (file_idx, piece_idx)));
                            }
                        }
                        Ordering::Equal => {
                            file_pieces.clear();
                            if file_min_possible_owners < shared_file.num_peers_with_state() {
                                file_pieces
                                    .extend(pieces.iter().map(|&piece_idx| (file_idx, piece_idx)));
                            }
                        }
                        Ordering::Greater => {}
                    }
                }
            }

            if file_pieces.is_empty() {
                return;
            }

            while num_pieces_to_be_sent > 0 && file_pieces.len() > 0 {
                let idx = rng.gen_range(0..file_pieces.len());
                let (file_idx, piece_idx) = file_pieces.swap_remove(idx);

                let mut shared_file = files[file_idx].write().await;
                let peer_id = shared_file
                    .select_piece_peer(piece_idx, current_time.clone())
                    .unwrap();
                let file = shared_file.file();

                let remote_peer = peers.get(&peer_id).unwrap();
                let message = PeerPeerMessage::FilePiece {
                    sha256: file.sha256(),
                    piece_idx,
                    bytes: file.get_piece(&piece_idx).unwrap().unwrap(),
                };
                match max_buffer_bytes {
                    Some(max_buffer_bytes) => {
                        let result =
                            remote_peer.send_with_max_buffer_size(message, max_buffer_bytes);
                        match result {
                            Ok(()) => {}
                            Err(PeerConnectionSendError::BufferIsFilled) => return,
                        }
                    }
                    None => remote_peer.send(message),
                };

                num_pieces_to_be_sent -= 1;
            }
        }
    }
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum LocalPeerAddFileError {
    #[error("file is already added")]
    AlreadyAdded,
}
