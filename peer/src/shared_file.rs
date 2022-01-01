use js_sys::Uint8Array;

use core::borrow::Borrow;
use std::collections::{BTreeMap, HashMap};

use thiserror::Error;
use tracker_protocol::PeerId;

use crate::{
    File, FileChunk, FilePieceData, FilePieceIdx, FilePiecesQueues, FileSetPieceError, FileState,
    PieceNumConfirmedOwners, PieceNumPossibleOwners, FILE_CHUNK_SIZE,
};

pub type JsSharedFile<T> = SharedFile<Uint8Array, T, FILE_CHUNK_SIZE>;

#[derive(Debug)]
pub struct SharedFile<C, T, const CHUNK_SIZE: usize> {
    /// File metadata and contents.
    file: File<C, CHUNK_SIZE>,

    /// File piece mask where pieces are owned by all remote devices.
    confirmed_remote_state: FileState,

    /// Peers states and statuses.
    peers: HashMap<PeerId, SharedFilePeer<T>>,

    /// PeerId ordered by PeerIdx.
    shared_peers_order: Vec<PeerId>,

    /// File pieces sharing data queues and cache.
    piece_queues: FilePiecesQueues,

    /// Pieces that have been sent and may not have been received.
    sent_pieces: BTreeMap<T, Vec<(PeerId, FilePieceIdx)>>,

    /// A list of recently received file pieces.
    recently_added_pieces: Vec<FilePieceIdx>,
}

#[derive(Clone, Debug)]
struct SharedFilePeer<T> {
    state: Option<SharedFilePeerState>,
    local_state_status: SharedFileLocalStateStatus<T>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedFileLocalStateStatus<T> {
    NotSent,
    Sent(T),
    Received,
}

#[derive(Clone, Debug)]
pub struct SharedFilePeerState {
    peer_idx: usize,
    confirmed: FileState,
    possible: FileState,
}

impl<C, T, const CHUNK_SIZE: usize> SharedFile<C, T, CHUNK_SIZE> {
    pub fn new(file: File<C, CHUNK_SIZE>) -> Self {
        let num_pieces = file.num_pieces();
        Self {
            file,
            confirmed_remote_state: FileState::from_complete(num_pieces),
            peers: HashMap::new(),
            shared_peers_order: Vec::new(),
            piece_queues: FilePiecesQueues::new(num_pieces),
            sent_pieces: BTreeMap::new(),
            recently_added_pieces: Vec::new(),
        }
    }

    pub fn file(&self) -> &File<C, CHUNK_SIZE> {
        &self.file
    }

    pub fn remote_state(&self) -> &FileState {
        &self.confirmed_remote_state
    }

    pub fn peer_ids(&self) -> impl Iterator<Item = &PeerId> {
        self.peers.keys()
    }

    pub fn num_peers_with_state(&self) -> PieceNumPossibleOwners {
        PieceNumPossibleOwners(self.shared_peers_order.len())
    }

    pub fn num_pieces(&self) -> usize {
        self.file.num_pieces()
    }

    pub fn piece_queues(&self) -> &FilePiecesQueues {
        &self.piece_queues
    }

    pub fn has_peer(&self, peer_id: PeerId) -> bool {
        self.peers.contains_key(&peer_id)
    }

    pub fn add_peer(&mut self, peer_id: PeerId) -> Result<(), SharedFileAddPeerError> {
        use std::collections::hash_map::Entry;

        let entry = match self.peers.entry(peer_id) {
            Entry::Occupied(_) => Err(SharedFileAddPeerError::PeerIsAlreadyAdded),
            Entry::Vacant(entry) => Ok(entry),
        }?;

        let _: &mut _ = entry.insert(SharedFilePeer {
            state: None,
            local_state_status: SharedFileLocalStateStatus::NotSent,
        });

        Ok(())
    }

    fn add_peer_state(
        &mut self,
        peer_id: PeerId,
        state: FileState,
    ) -> Result<(), SharedFileAddPeerStateError> {
        use crate::{PiecePeerShift, PushAndReturnOffset};

        let num_pieces = self.num_pieces();
        let peer = self
            .peers
            .get(&peer_id)
            .ok_or(SharedFileAddPeerStateError::PeerIsNotAdded)?;

        if peer.state.is_some() {
            return Err(SharedFileAddPeerStateError::PeerIsAlreadyHasState);
        }

        if state.len() != num_pieces {
            return Err(SharedFileAddPeerStateError::PeerInvalidStateLen {
                peer_len: state.len(),
                local_len: num_pieces,
            });
        }

        let local_state = self.file.state().raw().iter();
        let remote_state = self.confirmed_remote_state.raw().iter();
        let peer_state = state.raw().iter();
        let max_prev_owners = self.shared_peers_order.len();

        for (piece_idx, (local, (remote, peer))) in
            local_state.zip(remote_state.zip(peer_state)).enumerate()
        {
            let piece_idx = FilePieceIdx(piece_idx);
            match (*local, *remote, *peer) {
                // the piece is present locally and on all remote peers, except the added one
                (true, true, false) => {
                    let num_confirmed_owners = PieceNumConfirmedOwners(max_prev_owners);
                    let num_possible_owners = PieceNumPossibleOwners(max_prev_owners);
                    let piece = FilePieceData {
                        peer_shift: PiecePeerShift(0),
                        num_confirmed_owners,
                        num_possible_owners,
                    };
                    insert_piece(&mut self.piece_queues, &self.peers, piece_idx, piece);
                }
                // the piece is present locally and on the added peer, but not on all remote peers
                (true, false, true) => {
                    let mut piece = self.piece_queues.remove(&piece_idx).unwrap();
                    piece.num_confirmed_owners.0 += 1;
                    piece.num_possible_owners.0 += 1;
                    insert_piece(&mut self.piece_queues, &self.peers, piece_idx, piece);
                }
                (true, false, false) | (true, true, true) | (false, _, _) => {}
            }
        }

        self.confirmed_remote_state = self.confirmed_remote_state.clone() & &state;
        let peer_idx = self.shared_peers_order.push_and_get_offset(peer_id);

        let peer = self.peers.get_mut(&peer_id).unwrap();

        peer.state = Some(SharedFilePeerState {
            peer_idx,
            confirmed: state.clone(),
            possible: state,
        });

        Ok(())
    }

    pub fn remove_peer(&mut self, peer_id: &PeerId) -> Result<(), SharedFileRemovePeerError>
    where
        T: Ord,
    {
        match self.remove_peer_state(peer_id) {
            Ok(()) | Err(SharedFileRemovePeerStateError::PeerStateIsAlreadyRemoved) => Ok(()),
            Err(SharedFileRemovePeerStateError::PeerIsNotAdded) => {
                Err(SharedFileRemovePeerError::PeerIsNotAdded)
            }
        }?;

        let _ = self.peers.remove(&peer_id).unwrap();

        Ok(())
    }

    fn remove_peer_state(&mut self, peer_id: &PeerId) -> Result<(), SharedFileRemovePeerStateError>
    where
        T: Ord,
    {
        use core::mem::take;
        use core::ops::BitAnd;

        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or(SharedFileRemovePeerStateError::PeerIsNotAdded)?;
        let peer_state = match peer.state.take() {
            Some(peer_state) => peer_state,
            None => return Err(SharedFileRemovePeerStateError::PeerStateIsAlreadyRemoved),
        };

        let local_state = self.file.state().raw().iter();
        let remote_state = self.confirmed_remote_state.raw().iter();
        let _: PeerId = self.shared_peers_order.swap_remove(peer_state.peer_idx);
        let peer_state = peer_state.confirmed.raw().iter();

        for (piece_idx, (local, (remote, peer))) in
            local_state.zip(remote_state.zip(peer_state)).enumerate()
        {
            let piece_idx = FilePieceIdx(piece_idx);
            match (*local, *remote, *peer) {
                // the piece is present locally and on all remote peers, except the added one
                (true, true, false) => {
                    let _ = self.piece_queues.remove(&piece_idx).unwrap();
                }
                // the piece is present locally and on the added peer, but not on all remote peers
                (true, false, true) => {
                    let mut piece = self.piece_queues.remove(&piece_idx).unwrap();
                    piece.num_confirmed_owners.0 -= 1;
                    piece.num_possible_owners.0 -= 1;
                    insert_piece(&mut self.piece_queues, &self.peers, piece_idx, piece);
                }
                (true, false, false) | (true, true, true) | (false, _, _) => {}
            }
        }

        self.confirmed_remote_state = self
            .peers
            .values()
            .filter_map(|peer| peer.state.as_ref())
            .map(|peer| &peer.confirmed)
            .fold(FileState::from_complete(self.num_pieces()), BitAnd::bitand);

        self.sent_pieces = take(&mut self.sent_pieces)
            .into_iter()
            .map(|(duration, pieces)| {
                (
                    duration,
                    pieces
                        .into_iter()
                        .filter(|(other_peer_id, _)| peer_id != other_peer_id)
                        .collect(),
                )
            })
            .collect();

        Ok(())
    }

    pub fn set_peer_state(
        &mut self,
        peer_id: PeerId,
        state: FileState,
    ) -> Result<(), SharedFileSetPeerStateError>
    where
        T: Ord,
    {
        match self.remove_peer_state(&peer_id) {
            Ok(()) | Err(SharedFileRemovePeerStateError::PeerStateIsAlreadyRemoved) => Ok(()),
            Err(SharedFileRemovePeerStateError::PeerIsNotAdded) => {
                Err(SharedFileSetPeerStateError::PeerIsNotAdded)
            }
        }?;
        match self.add_peer_state(peer_id, state) {
            Ok(()) => Ok(()),
            Err(SharedFileAddPeerStateError::PeerIsNotAdded)
            | Err(SharedFileAddPeerStateError::PeerIsAlreadyHasState) => unreachable!(),
            Err(SharedFileAddPeerStateError::PeerInvalidStateLen {
                peer_len,
                local_len,
            }) => Err(SharedFileSetPeerStateError::PeerInvalidStateLen {
                peer_len,
                local_len,
            }),
        }
    }

    pub fn set_peer_file_missing(
        &mut self,
        peer_id: PeerId,
    ) -> Result<(), SharedFileSetPeerStateError>
    where
        T: Ord,
    {
        self.set_peer_state(peer_id, FileState::from_missing(self.num_pieces()))
    }

    pub fn set_peer_file_complete(
        &mut self,
        peer_id: PeerId,
    ) -> Result<(), SharedFileSetPeerStateError>
    where
        T: Ord,
    {
        self.set_peer_state(peer_id, FileState::from_complete(self.num_pieces()))
    }

    pub fn add_local_piece(
        &mut self,
        piece_idx: FilePieceIdx,
        data: &[u8],
    ) -> Result<(), SharedFileAddLocalPieceError>
    where
        C: FileChunk,
    {
        use crate::{FileStateSetStatus, PiecePeerShift};

        match self.file.set_piece(&piece_idx, data)? {
            FileStateSetStatus::AlreadySet => Err(SharedFileAddLocalPieceError::PieceIsAlreadySet),
            FileStateSetStatus::JustSet => Ok(()),
        }?;

        self.recently_added_pieces.push(piece_idx);

        let num_confirmed_owners = num_piece_confirmed_owners(&self.peers, &piece_idx);
        if num_confirmed_owners.0 == self.peers.len() {
            return Ok(());
        }

        let num_possible_owners = num_piece_possible_owners(&self.peers, &piece_idx);
        assert!(num_confirmed_owners.0 <= num_possible_owners.0);

        let piece = FilePieceData {
            peer_shift: PiecePeerShift(0),
            num_confirmed_owners: num_confirmed_owners,
            num_possible_owners: num_possible_owners,
        };
        insert_piece(&mut self.piece_queues, &self.peers, piece_idx, piece);

        Ok(())
    }

    pub fn take_recently_added_pieces(&mut self) -> Vec<FilePieceIdx> {
        use core::mem::take;

        take(&mut self.recently_added_pieces)
    }

    pub fn mark_pieces_for_resend_before(&mut self, time: T) -> Result<(), SharedFileMarkError>
    where
        T: Ord,
    {
        use core::mem::take;

        let mut not_sent = take(&mut self.sent_pieces);
        self.sent_pieces = not_sent.split_off(&time);

        for pieces in not_sent.into_values() {
            for (peer_id, piece_idx) in pieces {
                let _: SharedFileMarkForResendStatus =
                    self.mark_for_resend_if_not_sent(&peer_id, piece_idx)?;
            }
        }

        Ok(())
    }

    pub fn select_piece_peer(
        &mut self,
        piece_idx: FilePieceIdx,
        time: T,
    ) -> Result<PeerId, SharedFileSelectPiecePeerError>
    where
        T: Ord,
    {
        use crate::FileStateSetStatus;

        let piece_idx = check_piece_idx(piece_idx, self.num_pieces())
            .ok_or(SharedFileSelectPiecePeerError::PieceIndexOutOfRange)?;

        let num_peers = self.shared_peers_order.len();
        let mut piece = self.piece_queues.remove(&piece_idx).unwrap();

        let hash = fxhash::hash64(&piece_idx);
        let peer_idx_mult = ((hash >> 32) as usize % (num_peers - 1).max(1)) + 1;
        let peer_idx_offset = (hash & ((1 << 32) - 1)) as usize;

        let offset = |shift| ((peer_idx_mult * (peer_idx_offset + shift)) % num_peers) as usize;

        for shift in piece.peer_shift.0..piece.peer_shift.0 + num_peers {
            let peer_id = self.shared_peers_order[offset(shift)];
            println!(
                "{} {} {} {} {}",
                piece_idx.0,
                shift,
                piece.peer_shift.0,
                piece.peer_shift.0 + num_peers,
                peer_id.0
            );
            let peer = self.peers.get_mut(&peer_id).unwrap();
            let peer_state = peer.state.as_mut().unwrap();
            if peer_state.possible.set(&piece_idx).unwrap() == FileStateSetStatus::JustSet {
                piece.num_possible_owners.0 += 1;
                piece.peer_shift.0 = (shift + 1) % num_peers;
                insert_piece(&mut self.piece_queues, &self.peers, piece_idx, piece);
                self.sent_pieces
                    .entry(time)
                    .or_default()
                    .push((peer_id, piece_idx));
                return Ok(peer_id);
            }
        }
        Err(SharedFileSelectPiecePeerError::PieceIsAlreadyOwned)
    }

    pub fn mark_peer_piece_as_received_by_remote(
        &mut self,
        peer_id: &PeerId,
        piece_idx: FilePieceIdx,
    ) -> Result<SharedFileMarkStatus, SharedFileMarkError> {
        use crate::FileStateSetStatus;

        let num_pieces = self.num_pieces();
        let (state, piece_idx) =
            mark_peer_state_with_piece_idx(&mut self.peers, peer_id, piece_idx, num_pieces)?;

        let confirmed = state.confirmed.set(&piece_idx).unwrap();
        if confirmed == FileStateSetStatus::AlreadySet {
            return Ok(SharedFileMarkStatus::AlreadyMarked);
        }
        let possible = state.possible.set(&piece_idx).unwrap();

        if !self.file.has_piece(&piece_idx).unwrap() {
            return Ok(SharedFileMarkStatus::JustMarked);
        }

        let mut piece = self.piece_queues.remove(&piece_idx).unwrap();
        piece.num_confirmed_owners.0 += 1;
        if possible == FileStateSetStatus::JustSet {
            piece.num_possible_owners.0 += 1;
        }
        assert!(piece.num_confirmed_owners.0 <= piece.num_possible_owners.0);

        insert_piece(&mut self.piece_queues, &self.peers, piece_idx, piece);
        Ok(SharedFileMarkStatus::JustMarked)
    }

    pub fn mark_for_resend_if_not_sent(
        &mut self,
        peer_id: &PeerId,
        piece_idx: FilePieceIdx,
    ) -> Result<SharedFileMarkForResendStatus, SharedFileMarkError> {
        use crate::FileStateUnsetStatus;

        let num_pieces = self.num_pieces();
        let (state, piece_idx) =
            mark_peer_state_with_piece_idx(&mut self.peers, peer_id, piece_idx, num_pieces)?;

        if state.confirmed.has(&piece_idx).unwrap() {
            return Ok(SharedFileMarkForResendStatus::Received);
        }

        let possible = state.possible.unset(&piece_idx).unwrap();
        if possible == FileStateUnsetStatus::AlreadyUnset {
            return Ok(SharedFileMarkForResendStatus::AlreadyMarked);
        }
        let mut piece = self.piece_queues.remove(&piece_idx).unwrap();
        piece.num_possible_owners.0 -= 1;
        insert_piece(&mut self.piece_queues, &self.peers, piece_idx, piece);
        Ok(SharedFileMarkForResendStatus::JustMarked)
    }

    pub fn local_state_status(
        &mut self,
        peer_id: &PeerId,
    ) -> Result<&SharedFileLocalStateStatus<T>, LocalStateStatusError> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or(LocalStateStatusError::PeerIsNotAdded)?;

        Ok(&peer.local_state_status)
    }

    pub fn local_state_status_mut(
        &mut self,
        peer_id: &PeerId,
    ) -> Result<&mut SharedFileLocalStateStatus<T>, LocalStateStatusError> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or(LocalStateStatusError::PeerIsNotAdded)?;

        Ok(&mut peer.local_state_status)
    }
}

fn insert_piece<T>(
    pieces: &mut FilePiecesQueues,
    peers: &HashMap<PeerId, SharedFilePeer<T>>,
    piece_idx: FilePieceIdx,
    data: FilePieceData,
) {
    debug_assert_eq!(
        data.num_possible_owners,
        num_piece_possible_owners(peers, &piece_idx)
    );
    debug_assert_eq!(
        data.num_confirmed_owners,
        num_piece_confirmed_owners(peers, &piece_idx)
    );
    debug_assert!(data.num_confirmed_owners.0 <= data.num_possible_owners.0);
    let _ = pieces.insert(piece_idx, data);
}

fn num_piece_possible_owners<T>(
    peers: &HashMap<PeerId, SharedFilePeer<T>>,
    piece_idx: &FilePieceIdx,
) -> PieceNumPossibleOwners {
    PieceNumPossibleOwners(
        peers
            .values()
            .filter_map(|peer| peer.state.as_ref())
            .filter(|state| state.possible.has(piece_idx).unwrap())
            .count(),
    )
}

fn num_piece_confirmed_owners<T>(
    peers: &HashMap<PeerId, SharedFilePeer<T>>,
    piece_idx: &FilePieceIdx,
) -> PieceNumConfirmedOwners {
    PieceNumConfirmedOwners(
        peers
            .values()
            .filter_map(|peer| peer.state.as_ref())
            .filter(|state| state.confirmed.has(piece_idx).unwrap())
            .count(),
    )
}

fn mark_peer_state_with_piece_idx<'a, T, C>(
    peers: &'a mut HashMap<PeerId, SharedFilePeer<T>>,
    peer_id: &PeerId,
    piece_idx: C,
    num_pieces: usize,
) -> Result<(&'a mut SharedFilePeerState, C), SharedFileMarkError>
where
    C: Borrow<FilePieceIdx>,
{
    let piece_idx =
        check_piece_idx(piece_idx, num_pieces).ok_or(SharedFileMarkError::PieceIndexOutOfRange)?;

    let peer = peers
        .get_mut(peer_id)
        .ok_or(SharedFileMarkError::PeerIsNotAdded)?;

    let state = peer
        .state
        .as_mut()
        .ok_or(SharedFileMarkError::PeerStateIsNotAdded)?;

    Ok((state, piece_idx))
}

fn check_piece_idx<C>(piece_idx: C, num_pieces: usize) -> Option<C>
where
    C: Borrow<FilePieceIdx>,
{
    if piece_idx.borrow().0 < num_pieces {
        Some(piece_idx)
    } else {
        None
    }
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum SharedFileAddPeerError {
    #[error("peer is already added to SharedFile")]
    PeerIsAlreadyAdded,
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum SharedFileAddPeerStateError {
    #[error("peer is not added to SharedFile")]
    PeerIsNotAdded,
    #[error("peer is already has a state")]
    PeerIsAlreadyHasState,
    #[error("peer state length: {peer_len}, not matched local state length: {local_len}")]
    PeerInvalidStateLen { peer_len: usize, local_len: usize },
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum SharedFileRemovePeerError {
    #[error("peer is not added to SharedFile")]
    PeerIsNotAdded,
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum SharedFileRemovePeerStateError {
    #[error("peer is not added to SharedFile")]
    PeerIsNotAdded,
    #[error("peer state is already removed")]
    PeerStateIsAlreadyRemoved,
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum SharedFileSetPeerStateError {
    #[error("peer is not added to SharedFile")]
    PeerIsNotAdded,
    #[error("peer state length: {peer_len}, not matched local state length: {local_len}")]
    PeerInvalidStateLen { peer_len: usize, local_len: usize },
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum SharedFileSelectPiecePeerError {
    #[error("piece index out of range")]
    PieceIndexOutOfRange,
    #[error("piece is already owned by all peers")]
    PieceIsAlreadyOwned,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SharedFileMarkStatus {
    JustMarked,
    AlreadyMarked,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SharedFileMarkForResendStatus {
    Received,
    JustMarked,
    AlreadyMarked,
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum SharedFileMarkError {
    #[error("peer is not added to SharedFile")]
    PeerIsNotAdded,
    #[error("piece index out of range")]
    PieceIndexOutOfRange,
    #[error("peer state is not added to SharedFile")]
    PeerStateIsNotAdded,
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum SharedFileAddLocalPieceError {
    #[error(transparent)]
    SetPiece(#[from] FileSetPieceError),
    #[error("piece is already set")]
    PieceIsAlreadySet,
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum LocalStateStatusError {
    #[error("peer is not added to SharedFile")]
    PeerIsNotAdded,
}

#[test]
fn send_shared_file_to_single_receiver() {
    use crate::{FileLen, FileMetadata, FILE_PIECE_SIZE};
    use bitvec::bits;
    use bitvec::store::BitStore;
    use tracker_protocol::FileSha256;

    const NUM_PIECES: usize = 16;
    const CHUNK_LEN: usize = FILE_PIECE_SIZE * 2;

    let metadata = FileMetadata::new(
        FileSha256(Default::default()),
        "filename".to_owned(),
        FileLen((NUM_PIECES * FILE_PIECE_SIZE) as u64),
    );
    let file: File<Box<[u8]>, CHUNK_LEN> = File::new(metadata).unwrap();
    let mut shared_file = SharedFile::new(file);

    assert_eq!(shared_file.file().state().raw(), bits![0; NUM_PIECES]);
    assert_eq!(shared_file.file().state().num_missing(), NUM_PIECES);
    assert_eq!(shared_file.file().state().num_available(), 0);
    assert_eq!(shared_file.file().state().is_missing(), true);
    assert_eq!(shared_file.file().state().is_complete(), false);
    assert_eq!(shared_file.num_pieces(), NUM_PIECES);
    assert_eq!(shared_file.peer_ids().count(), 0);
    assert_eq!(shared_file.piece_queues().next_queue(), None);

    for j in 0..NUM_PIECES {
        shared_file
            .add_local_piece(FilePieceIdx(j), &[0; FILE_PIECE_SIZE])
            .unwrap();
    }
    let _ = shared_file.take_recently_added_pieces();

    assert_eq!(shared_file.file().state().raw(), bits![1; NUM_PIECES]);
    assert_eq!(shared_file.file().state().num_missing(), 0);
    assert_eq!(shared_file.file().state().num_available(), NUM_PIECES);
    assert_eq!(shared_file.file().state().is_missing(), false);
    assert_eq!(shared_file.file().state().is_complete(), true);
    assert_eq!(shared_file.num_pieces(), NUM_PIECES);
    assert_eq!(shared_file.peer_ids().count(), 0);
    assert_eq!(shared_file.piece_queues().next_queue(), None);

    shared_file.add_peer(PeerId(1)).unwrap();
    assert_eq!(
        shared_file.local_state_status(&PeerId(1)).unwrap(),
        &SharedFileLocalStateStatus::NotSent
    );
    assert_eq!(shared_file.peer_ids().count(), 1);
    assert_eq!(shared_file.piece_queues().next_queue(), None);

    let get_queue_num_owners = |file: &SharedFile<Box<[u8]>, i32, CHUNK_LEN>| {
        file.piece_queues().next_queue().unwrap().0 .0
    };
    let get_queue = |file: &SharedFile<Box<[u8]>, i32, CHUNK_LEN>| {
        file.piece_queues()
            .next_queue()
            .unwrap()
            .1
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>()
    };

    shared_file
        .add_peer_state(PeerId(1), FileState::from_missing(NUM_PIECES))
        .unwrap();
    assert_eq!(shared_file.peer_ids().count(), 1);
    assert_eq!(get_queue_num_owners(&shared_file), 0);
    assert_eq!(
        get_queue(&shared_file),
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
    );

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(4), 0).unwrap();
    assert_eq!(peer_id, PeerId(1));
    assert_eq!(get_queue_num_owners(&shared_file), 0);
    assert_eq!(
        get_queue(&shared_file),
        &[0, 1, 2, 3, 15, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(7), 0).unwrap();
    assert_eq!(peer_id, PeerId(1));
    assert_eq!(get_queue_num_owners(&shared_file), 0);
    assert_eq!(
        get_queue(&shared_file),
        &[0, 1, 2, 3, 15, 5, 6, 14, 8, 9, 10, 11, 12, 13]
    );

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(13), 0).unwrap();
    assert_eq!(peer_id, PeerId(1));
    assert_eq!(get_queue_num_owners(&shared_file), 0);
    assert_eq!(
        get_queue(&shared_file),
        &[0, 1, 2, 3, 15, 5, 6, 14, 8, 9, 10, 11, 12]
    );

    for j in [6, 2, 5, 8, 14, 10, 9, 11, 0, 3, 15, 12] {
        let peer_id = shared_file.select_piece_peer(FilePieceIdx(j), 0).unwrap();
        assert_eq!(peer_id, PeerId(1));
        assert_eq!(get_queue_num_owners(&shared_file), 0);
        println!("{} {:?}", j, get_queue(&shared_file));
    }
    assert_eq!(get_queue(&shared_file), &[1]);

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(1), 0).unwrap();
    assert_eq!(peer_id, PeerId(1));
    assert_eq!(get_queue_num_owners(&shared_file), 1);
    assert_eq!(
        get_queue(&shared_file),
        &[4, 7, 13, 6, 2, 5, 8, 14, 10, 9, 11, 0, 3, 15, 12, 1]
    );

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(6), 0);
    assert_eq!(
        peer_id,
        Err(SharedFileSelectPiecePeerError::PieceIsAlreadyOwned)
    );
}

#[test]
fn send_shared_file_to_multiple_receivers() {
    use crate::{FileLen, FileMetadata, FILE_PIECE_SIZE};
    use tracker_protocol::FileSha256;

    const NUM_PIECES: usize = 4;
    const CHUNK_LEN: usize = FILE_PIECE_SIZE * 2;

    let metadata = FileMetadata::new(
        FileSha256(Default::default()),
        "filename".to_owned(),
        FileLen((NUM_PIECES * FILE_PIECE_SIZE) as u64),
    );
    let file: File<Box<[u8]>, CHUNK_LEN> = File::new(metadata).unwrap();
    let mut shared_file: SharedFile<_, i32, CHUNK_LEN> = SharedFile::new(file);

    for j in 0..NUM_PIECES {
        shared_file
            .add_local_piece(FilePieceIdx(j), &[0; FILE_PIECE_SIZE])
            .unwrap();
    }
    let _ = shared_file.take_recently_added_pieces();

    for j in 1..=8 {
        shared_file.add_peer(PeerId(j)).unwrap();
        assert_eq!(
            shared_file.local_state_status(&PeerId(1)).unwrap(),
            &SharedFileLocalStateStatus::NotSent
        );
        assert_eq!(shared_file.peer_ids().count(), j as usize);

        shared_file
            .add_peer_state(PeerId(j), FileState::from_missing(NUM_PIECES))
            .unwrap();
        assert_eq!(shared_file.peer_ids().count(), j as usize);
    }

    let get_queue_num_owners = |file: &SharedFile<Box<[u8]>, i32, CHUNK_LEN>| {
        for j in 1..=8 {
            let peer = file.peers.get(&PeerId(j)).unwrap();
            print!(
                "{} ",
                peer.state
                    .as_ref()
                    .unwrap()
                    .possible
                    .raw()
                    .iter()
                    .map(|bit| if *bit { '+' } else { '-' })
                    .collect::<String>()
            );
        }
        println!();
        file.piece_queues().next_queue().unwrap().0 .0
    };
    let get_queue = |file: &SharedFile<Box<[u8]>, i32, CHUNK_LEN>| {
        file.piece_queues()
            .next_queue()
            .unwrap()
            .1
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>()
    };

    assert_eq!(get_queue_num_owners(&shared_file), 0);
    assert_eq!(get_queue(&shared_file), &[0, 1, 2, 3]);

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(1), 0).unwrap();
    assert_eq!(peer_id, PeerId(5));
    assert_eq!(get_queue_num_owners(&shared_file), 0);
    assert_eq!(get_queue(&shared_file), &[0, 3, 2]);

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(0), 0).unwrap();
    assert_eq!(peer_id, PeerId(1));
    assert_eq!(get_queue_num_owners(&shared_file), 0);
    assert_eq!(get_queue(&shared_file), &[2, 3]);

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(3), 0).unwrap();
    assert_eq!(peer_id, PeerId(6));
    assert_eq!(get_queue_num_owners(&shared_file), 0);
    assert_eq!(get_queue(&shared_file), &[2]);

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(2), 0).unwrap();
    assert_eq!(peer_id, PeerId(7));
    assert_eq!(get_queue_num_owners(&shared_file), 1);
    assert_eq!(get_queue(&shared_file), &[1, 0, 3, 2]);

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(2), 0).unwrap();
    assert_eq!(peer_id, PeerId(6));
    assert_eq!(get_queue_num_owners(&shared_file), 1);
    assert_eq!(get_queue(&shared_file), &[1, 0, 3]);

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(1), 0).unwrap();
    assert_eq!(peer_id, PeerId(1));
    assert_eq!(get_queue_num_owners(&shared_file), 1);
    assert_eq!(get_queue(&shared_file), &[3, 0]);

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(3), 0).unwrap();
    assert_eq!(peer_id, PeerId(1));
    assert_eq!(get_queue_num_owners(&shared_file), 1);
    assert_eq!(get_queue(&shared_file), &[0]);

    let peer_id = shared_file.select_piece_peer(FilePieceIdx(0), 0).unwrap();
    assert_eq!(peer_id, PeerId(2));
    assert_eq!(get_queue_num_owners(&shared_file), 2);
    assert_eq!(get_queue(&shared_file), &[2, 1, 3, 0]);
}
