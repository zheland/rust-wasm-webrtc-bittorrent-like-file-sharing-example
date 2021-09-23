use core::sync::atomic::AtomicU32;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

use async_std::sync::{Mutex, RwLock};
use protocol::{FileSha256, PeerId};
use thiserror::Error;

use crate::SocketSender;

#[derive(Debug)]
pub struct State {
    peers_senders: RwLock<HashMap<PeerId, Weak<Mutex<SocketSender>>>>,
    files_senders: RwLock<HashMap<FileSha256, Arc<RwLock<HashSet<PeerId>>>>>,
    next_peer_id: AtomicU32,
}

impl State {
    pub fn new() -> Self {
        Self {
            peers_senders: RwLock::new(HashMap::new()),
            files_senders: RwLock::new(HashMap::new()),
            next_peer_id: AtomicU32::new(0),
        }
    }

    pub async fn new_peer(&self, sender: &Arc<Mutex<SocketSender>>) -> PeerId {
        use core::sync::atomic::Ordering;

        let peer_idx = self.next_peer_id.fetch_add(1, Ordering::Relaxed);
        let peer_id = PeerId(peer_idx);
        let prev = self
            .peers_senders
            .write()
            .await
            .insert(peer_id, Arc::downgrade(sender));
        assert!(prev.is_none());

        peer_id
    }

    pub async fn get_peer_sender(&self, peer_id: PeerId) -> Option<Arc<Mutex<SocketSender>>> {
        self.peers_senders
            .read()
            .await
            .get(&peer_id)
            .and_then(Weak::upgrade)
    }

    pub async fn add_file_peer_and_get_file_peer_list(
        &self,
        file_sha256: FileSha256,
        peer_id: PeerId,
    ) -> Result<Vec<PeerId>, StateAddFilePeerError> {
        let file_peers = self.get_or_insert_default_file_peers(file_sha256).await;
        let mut file_peers = file_peers.write().await;

        let is_inserted = file_peers.insert(peer_id);
        if is_inserted {
            Ok(file_peers.iter().copied().collect())
        } else {
            Err(StateAddFilePeerError::FileIsAlreadyAdded(file_sha256))
        }
    }

    async fn get_or_insert_default_file_peers(
        &self,
        file_sha256: FileSha256,
    ) -> Arc<RwLock<HashSet<PeerId>>> {
        let mut files_peers = self.files_senders.write().await;
        let file_peers = files_peers.get(&file_sha256);
        match file_peers {
            Some(file_peers) => Arc::clone(file_peers),
            None => {
                let file_peers = Arc::new(RwLock::new(HashSet::new()));
                let prev = files_peers.insert(file_sha256, Arc::clone(&file_peers));
                assert!(prev.is_none());
                file_peers
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum StateAddFilePeerError {
    #[error("file {0} is already added")]
    FileIsAlreadyAdded(FileSha256),
}
