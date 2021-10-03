use std::collections::{HashMap, HashSet};

use thiserror::Error;
use tracker_protocol::PeerId;

use crate::{FilePieceIdx, FilePiecesQueues, FileState};

type PeerIdx = usize;

#[derive(Clone, Debug)]
pub struct FileSharingState {
    local_state: FileState,
    remote_state: FileState,

    peers: Vec<FileSharingPeerData>,
    peers_map: HashMap<PeerId, PeerIdx>,
    pieces: FilePiecesQueues,
}

#[derive(Clone, Debug)]
pub struct FileSharingPeerData {
    peer_id: PeerId,
    state: FileState,
    sends_in_progress: HashSet<FilePieceIdx>,
}

impl FileSharingState {
    pub fn new(local_state: FileState) -> Self {
        let num_pieces = local_state.len();
        Self {
            local_state,
            remote_state: FileState::new_complete(num_pieces),
            peers: Vec::new(),
            peers_map: HashMap::new(),
            pieces: FilePiecesQueues::new(num_pieces),
        }
    }

    pub fn remote_state(&self) -> &FileState {
        &self.remote_state
    }

    pub fn local_state(&self) -> &FileState {
        &self.local_state
    }

    pub fn pieces(&self) -> &FilePiecesQueues {
        &self.pieces
    }

    pub fn num_pieces(&self) -> usize {
        self.local_state.len()
    }

    pub fn has_peer(&self, peer_id: PeerId) -> bool {
        self.peers_map.contains_key(&peer_id)
    }

    pub fn add_peer(
        &mut self,
        peer_id: PeerId,
        state: FileState,
    ) -> Result<(), FileSharingStateAddPeerError> {
        use crate::{FilePieceData, PushAndReturnOffset};
        use core::mem::replace;
        use std::collections::hash_map::Entry;

        let num_peers_before = self.peers.len();
        let num_pieces = self.num_pieces();
        let peer_idx = self.peers_map.entry(peer_id);
        let peer_idx = match peer_idx {
            Entry::Occupied(_) => Err(FileSharingStateAddPeerError::PeerIsAlreadyAdded { peer_id }),
            Entry::Vacant(peer_idx) => Ok(peer_idx),
        }?;

        if state.len() != num_pieces {
            return Err(FileSharingStateAddPeerError::PeerInvalidStateLen {
                peer_len: state.len(),
                local_len: num_pieces,
            });
        }

        for (piece_idx, (local, (remote, peer))) in self
            .local_state
            .raw()
            .iter()
            .zip(self.remote_state.raw().iter().zip(state.raw().iter()))
            .enumerate()
        {
            let piece_idx = piece_idx.into();
            match (*local, *remote, *peer) {
                (true, true, false) => {
                    let piece = FilePieceData {
                        idx: piece_idx,
                        peer_shift: 0,
                        num_owners: num_peers_before as u32,
                        send_attempts: 0,
                    };
                    self.pieces.add(piece).unwrap();
                }
                (true, false, true) => {
                    let mut piece = self.pieces.remove(piece_idx).unwrap();
                    piece.num_owners += 1;
                    self.pieces.add(piece).unwrap();
                }
                (true, false, false) | (true, true, true) | (false, _, _) => {}
            }
        }

        let remote_state = replace(&mut self.remote_state, FileState::empty());
        self.remote_state = remote_state & &state;

        let data = FileSharingPeerData {
            peer_id,
            state,
            sends_in_progress: HashSet::new(),
        };
        let offset = self.peers.push_and_get_offset(data);
        let _: &mut _ = peer_idx.insert(offset);

        Ok(())
    }

    pub fn remove_peer(&mut self, peer_id: PeerId) -> Result<(), FileSharingStateRemovePeerError> {
        use core::ops::BitAnd;

        let peer_idx = self.peers_map.remove(&peer_id);
        let peer_idx =
            peer_idx.ok_or_else(|| FileSharingStateRemovePeerError::PeerIsNotAdded { peer_id })?;

        let peer = self.peers.swap_remove(peer_idx);
        if let Some(moved_peer) = self.peers.get(peer_idx) {
            let _ = self.peers_map.insert(moved_peer.peer_id, peer_idx);
        }

        self.remote_state = self
            .peers
            .iter()
            .map(|peer| &peer.state)
            .fold(FileState::new_complete(self.num_pieces()), BitAnd::bitand);

        for (piece_idx, (local, (remote, peer))) in self
            .local_state
            .raw()
            .iter()
            .zip(self.remote_state.raw().iter().zip(peer.state.raw().iter()))
            .enumerate()
        {
            let piece_idx = piece_idx.into();
            match (*local, *remote, *peer) {
                (true, true, false) => {
                    let _ = self.pieces.remove(piece_idx).unwrap();
                }
                (true, false, true) => {
                    let mut piece = self.pieces.remove(piece_idx).unwrap();
                    piece.num_owners -= 1;
                    self.pieces.add(piece).unwrap();
                }
                (true, false, false) | (true, true, true) | (false, _, _) => {}
            }
        }

        Ok(())
    }

    fn check_piece_idx(
        piece_idx: FilePieceIdx,
        len: usize,
    ) -> Result<FilePieceIdx, FileSharingStatePieceIndexError> {
        if usize::from(piece_idx) < len {
            Ok(piece_idx)
        } else {
            Err(FileSharingStatePieceIndexError::PieceIndexOutOfRange { piece_idx, len })
        }
    }

    pub(crate) fn select_peer(
        &mut self,
        piece_idx: FilePieceIdx,
    ) -> Result<PeerId, FileSharingSelectPeerError> {
        let piece_idx = Self::check_piece_idx(piece_idx, self.num_pieces())?;

        let num_peers = self.peers.len() as u32;
        let mut piece = self.pieces.remove(piece_idx).unwrap();

        let hash = fxhash::hash64(&piece_idx);
        let peer_idx_mult1 = ((hash >> 32) as u32 % num_peers) + 1;
        let peer_idx_mult2 = (hash & ((1 << 32) - 1)) as u32;

        let offset = |shift| ((peer_idx_mult1 * (peer_idx_mult2 + shift)) % num_peers) as usize;

        for shift in piece.peer_shift..piece.peer_shift + num_peers {
            let peer = &mut self.peers[offset(shift)];
            if !peer.state.has(piece_idx).unwrap() {
                let peer_id = peer.peer_id;
                let is_added = peer.sends_in_progress.insert(piece_idx);
                if is_added {
                    piece.send_attempts = piece.send_attempts.saturating_add(1);
                }
                piece.peer_shift = (shift + 1) % num_peers;
                self.pieces.add(piece).unwrap();
                return Ok(peer_id);
            }
        }
        Err(FileSharingSelectPeerError::PieceIsAlreadyOwned { piece_idx })
    }

    pub fn add_local_piece(
        &mut self,
        piece_idx: FilePieceIdx,
    ) -> Result<(), FileSharingStateAddLocalPieceError> {
        use crate::{FilePieceData, PieceNumOwners};

        let piece_idx = Self::check_piece_idx(piece_idx, self.num_pieces())?;
        let status = self.local_state.set(piece_idx.into()).unwrap();
        if !status.is_just_set() {
            return Err(FileSharingStateAddLocalPieceError::PieceIsAlreadySet { piece_idx });
        }

        let num_owners = self
            .peers
            .iter()
            .filter(|peer| peer.state.has(piece_idx).unwrap())
            .count();

        if num_owners < self.peers.len() {
            let piece = FilePieceData {
                idx: piece_idx,
                peer_shift: 0,
                num_owners: num_owners as PieceNumOwners,
                send_attempts: 0,
            };
            self.pieces.add(piece).unwrap();
        }
        Ok(())
    }

    pub fn set_peer_pieces_received(
        &mut self,
        peer_id: PeerId,
        piece_idxs: Vec<FilePieceIdx>,
    ) -> Result<(), FileSharingSetPeerPiecesReceivedError> {
        let peer_idx = *self
            .peers_map
            .get(&peer_id)
            .ok_or_else(|| FileSharingSetPeerPiecesReceivedError::PeerIsNotAdded { peer_id })?;
        let num_pieces = self.num_pieces();
        let peer = &mut self.peers[peer_idx];

        for piece_idx in piece_idxs {
            let piece_idx = Self::check_piece_idx(piece_idx, num_pieces)?;

            let has_piece = peer.state.has(piece_idx).unwrap();
            if has_piece {
                continue;
            }

            let is_sent = peer.sends_in_progress.remove(&piece_idx);
            let mut piece = self.pieces.remove(piece_idx).unwrap();
            if is_sent {
                piece.send_attempts = piece.send_attempts.checked_sub(1).unwrap();
            }
            piece.num_owners += 1;
            self.pieces.add(piece).unwrap();
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileSharingStateAddPeerError {
    #[error("peer {peer_id} is already added to FileStaringState")]
    PeerIsAlreadyAdded { peer_id: PeerId },
    #[error("peer state length: {peer_len}, not matched local state length: {local_len}")]
    PeerInvalidStateLen { peer_len: usize, local_len: usize },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileSharingStateRemovePeerError {
    #[error("peer {peer_id} not added to FileStaringState")]
    PeerIsNotAdded { peer_id: PeerId },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileSharingSelectPeerError {
    #[error(transparent)]
    PieceIndexError(#[from] FileSharingStatePieceIndexError),
    #[error("piece {piece_idx} is already owned by all peers")]
    PieceIsAlreadyOwned { piece_idx: FilePieceIdx },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileSharingSetPeerPiecesReceivedError {
    #[error("peer {peer_id} not added to FileStaringState")]
    PeerIsNotAdded { peer_id: PeerId },
    #[error(transparent)]
    PieceIndexError(#[from] FileSharingStatePieceIndexError),
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileSharingStateAddLocalPieceError {
    #[error(transparent)]
    PieceIndexError(#[from] FileSharingStatePieceIndexError),
    #[error("piece {piece_idx} is already set")]
    PieceIsAlreadySet { piece_idx: FilePieceIdx },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileSharingStatePieceIndexError {
    #[error("piece index {piece_idx} out of range for piece count {len}")]
    PieceIndexOutOfRange { piece_idx: FilePieceIdx, len: usize },
}

/*

#[derive(Error, Debug)]
pub enum FileSharingStateOnRemoteStateError {
    #[error("peer {peer_id} not added to FileStaringState")]
    PeerIsNotAdded { peer_id: PeerId },
    #[error("peer state length: {peer_len}, not matched local state length: {local_len}")]
    PeerInvalidStateLen { peer_len: usize, local_len: usize },
}

#[derive(Error, Debug)]
pub enum FileSharingStateOnRemotePieceReceivedError {
    #[error("peer {peer_id} not added to FileStaringState")]
    PeerIsNotAdded { peer_id: PeerId },
    #[error("piece_idx {piece_idx} does not exist in FileStaringState")]
    PieceDoesNotExist { piece_idx: FilePieceIdx },
}
*/
