use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use js_sys::Uint8Array;
use thiserror::Error;
use tracker_protocol::{FileSha256, PeerId};
use web_sys::{Blob, File as WebSysFile};

use crate::{
    FileLen, FileMetaData, FilePieceIdx, FileSharingState, FileSharingStateAddLocalPieceError,
    FileState, FileStateHasPieceError, NowError, RemotePeer, FILE_PIECE_SIZE,
};

// Chrome does not support creating an array buffer of 2 GB or more.
// Blob chunk size equal to 1MB is selected
// in order to optimize the process of file preparation for sharing
// and to reduce the probability of lags.
pub const FILE_CHUNK_SIZE: usize = 1048576;

const NUM_PIECES_IN_CHUNK: usize = FILE_CHUNK_SIZE / FILE_PIECE_SIZE;
static_assertions::const_assert_eq!(FILE_CHUNK_SIZE % FILE_PIECE_SIZE, 0);

#[derive(Debug)]
pub struct LocalFile {
    metadata: FileMetaData,
    chunks: Vec<Uint8Array>,
    num_pieces: usize,
    sharing_state: Arc<RwLock<FileSharingState>>,
    remote_status: RwLock<HashMap<PeerId, RemoteStateStatus>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteStateStatus {
    Received,
    Sent(Duration),
    NotSent,
}

impl LocalFile {
    pub fn new(metadata: FileMetaData) -> Result<Arc<Self>, NewFileError> {
        use core::cmp::min;

        pub const FILE_PIECE_SIZE_U64: u64 = FILE_PIECE_SIZE as u64;
        pub const FILE_CHUNK_SIZE_U64: u64 = FILE_CHUNK_SIZE as u64;

        let len = metadata.len();
        let num_chunks: u64 = (len + FILE_CHUNK_SIZE_U64 - 1) / FILE_CHUNK_SIZE_U64;
        let num_pieces = (len + FILE_PIECE_SIZE_U64 - 1) / FILE_PIECE_SIZE_U64;
        let num_pieces: usize = num_pieces
            .try_into()
            .map_err(|_| NewFileError::SizeIsTooLarge { len })?;

        let chunks = (0..num_chunks)
            .map(|j| {
                Uint8Array::new_with_length(
                    min(len - j * FILE_CHUNK_SIZE_U64, FILE_CHUNK_SIZE_U64)
                        .try_into()
                        .unwrap(),
                )
            })
            .collect();

        let state = FileState::new_missing(num_pieces);
        let sharing_state = FileSharingState::new(state);

        Ok(Arc::new(Self {
            metadata,
            chunks,
            num_pieces,
            sharing_state: Arc::new(RwLock::new(sharing_state)),
            remote_status: RwLock::new(HashMap::new()),
        }))
    }

    pub async fn from_file(
        file: WebSysFile,
        //mut progress_cb: F,
    ) -> Result<Arc<Self>, FileFromError>
where
        //F: FnMut(FileLen, FileLen) -> R,
        //R: Future<Output = ()>,
    {
        log::debug!("adding file {} ...", file.name());
        use js_sys::{ArrayBuffer, Number};
        use sha2::{Digest, Sha256};
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;

        pub const FILE_PIECE_SIZE_U64: u64 = FILE_PIECE_SIZE as u64;
        pub const FILE_CHUNK_SIZE_U64: u64 = FILE_CHUNK_SIZE as u64;

        let name = file.name();
        let blob = file.slice().unwrap();
        let len = blob.size();

        if !Number::is_safe_integer(&Number::from(len)) {
            return Err(FileFromError::SizeIsTooLarge {
                len: len as FileLen,
            });
        }
        let len = len as u64;
        let num_pieces = (len + FILE_PIECE_SIZE_U64 - 1) / FILE_PIECE_SIZE_U64;
        let num_pieces: usize = num_pieces
            .try_into()
            .map_err(|_| FileFromError::SizeIsTooLarge { len })?;

        let mut chunks = Vec::new();
        let mut hasher = Sha256::new();
        for start in (0..len).step_by(FILE_CHUNK_SIZE) {
            //if start > 0 {
            //    progress_cb(start, len).await;
            //}
            let end = start + FILE_CHUNK_SIZE_U64;
            let chunk = blob
                .slice_with_f64_and_f64(start as f64, end as f64)
                .unwrap();

            let array_buffer: ArrayBuffer = JsFuture::from(chunk.array_buffer())
                .await
                .unwrap()
                .dyn_into()
                .unwrap();

            let u8_array = Uint8Array::new(&array_buffer);
            hasher.update(&u8_array.to_vec());
            chunks.push(u8_array);
            log::debug!("adding file {} ... {}/{}bytes", file.name(), start, len);
        }

        let sha256 = FileSha256(hasher.finalize().into());
        let metadata = FileMetaData::new(sha256, name, len);

        let state = FileState::new_complete(num_pieces);
        let sharing_state = FileSharingState::new(state);

        log::debug!("adding file {} ... OK", file.name());
        Ok(Arc::new(Self {
            metadata,
            chunks,
            num_pieces,
            sharing_state: Arc::new(RwLock::new(sharing_state)),
            remote_status: RwLock::new(HashMap::new()),
        }))
    }

    pub async fn to_blob(&self) -> Result<Blob, FileToBlobError> {
        use js_sys::Array;

        let state = self.sharing_state.read().await;
        let state = state.local_state();
        if state.is_complete() {
            let blob_args: Array = self.chunks.iter().collect();
            Ok(Blob::new_with_u8_array_sequence(&blob_args).unwrap())
        } else {
            Err(FileToBlobError::NotComplete {
                available: state.num_available(),
                missing: state.num_missing(),
            })
        }
    }

    pub fn metadata(&self) -> &FileMetaData {
        &self.metadata
    }

    pub fn name(&self) -> &str {
        &self.metadata.name()
    }

    pub fn len(&self) -> FileLen {
        self.metadata.len()
    }

    pub fn sha256(&self) -> FileSha256 {
        self.metadata.sha256()
    }

    pub fn num_pieces(&self) -> usize {
        self.num_pieces
    }

    pub async fn sharing_state(&self) -> RwLockReadGuard<'_, FileSharingState> {
        self.sharing_state.read().await
    }

    pub async fn sharing_state_mut(&self) -> RwLockWriteGuard<'_, FileSharingState> {
        self.sharing_state.write().await
    }

    pub async fn remote_status(&self) -> RwLockReadGuard<'_, HashMap<PeerId, RemoteStateStatus>> {
        self.remote_status.read().await
    }

    pub async fn has_piece(&self, piece_idx: FilePieceIdx) -> Result<bool, FileHasPieceError> {
        Ok(self
            .sharing_state
            .read()
            .await
            .local_state()
            .has(piece_idx)?)
    }

    pub async fn get_piece(
        &self,
        piece_idx: FilePieceIdx,
    ) -> Result<Option<Vec<u8>>, FileGetPieceError> {
        let chunk_idx = usize::from(piece_idx) / NUM_PIECES_IN_CHUNK;
        let chunk_piece_idx = usize::from(piece_idx) % NUM_PIECES_IN_CHUNK;

        let has_piece = self
            .sharing_state
            .read()
            .await
            .local_state()
            .has(piece_idx)?;
        if has_piece {
            let chunk = &self.chunks[chunk_idx];
            let start = chunk_piece_idx * FILE_PIECE_SIZE;
            let end = start + FILE_PIECE_SIZE;
            let vec = chunk
                .slice(start.try_into().unwrap(), end.try_into().unwrap())
                .to_vec();
            Ok(Some(vec))
        } else {
            Ok(None)
        }
    }

    async fn try_set_piece(
        &self,
        piece_idx: FilePieceIdx,
        data: &[u8],
    ) -> Result<(), FileSetPieceError> {
        let chunk_idx = usize::from(piece_idx) / NUM_PIECES_IN_CHUNK;
        let chunk_piece_idx = usize::from(piece_idx) % NUM_PIECES_IN_CHUNK;

        let chunk = &self.chunks[chunk_idx];
        let start = chunk_piece_idx * FILE_PIECE_SIZE;
        let end = start + FILE_PIECE_SIZE;
        let slice = chunk.subarray(start.try_into().unwrap(), end.try_into().unwrap());
        let len = data.len();
        let expected = slice.length().try_into().unwrap();

        if len == expected {
            slice.copy_from(data);
            self.sharing_state
                .write()
                .await
                .add_local_piece(piece_idx)?;
            Ok(())
        } else {
            Err(FileSetPieceError::InvalidPieceLen {
                piece_idx,
                len,
                expected,
            })
        }
    }

    pub async fn add_remote(self: &Arc<Self>, peer_id: PeerId) {
        let mut status = self.remote_status.write().await;
        let remote = status.entry(peer_id);
        let _: &mut _ = remote.or_insert(RemoteStateStatus::NotSent);
    }

    pub async fn set_remote_file_missing(self: &Arc<Self>, peer_id: PeerId) {
        self.set_remote_file_state(peer_id, FileState::new_missing(self.num_pieces))
            .await;
    }

    pub async fn set_remote_file_complete(self: &Arc<Self>, peer_id: PeerId) {
        self.set_remote_file_state(peer_id, FileState::new_complete(self.num_pieces))
            .await;
    }

    pub async fn set_remote_file_state(self: &Arc<Self>, peer_id: PeerId, state: FileState) {
        use crate::{IgnoreEmpty, OkOrLog};

        let mut sharing = self.sharing_state.write().await;
        sharing.remove_peer(peer_id).ok().ignore_empty();
        sharing.add_peer(peer_id, state).ok_or_log().ignore_empty();
    }

    pub async fn on_state_received_by_remote(self: &Arc<Self>, peer_id: PeerId) {
        let _: Option<_> = self
            .remote_status
            .write()
            .await
            .insert(peer_id, RemoteStateStatus::Received);
    }

    pub async fn set_file_piece(self: &Arc<Self>, piece_idx: FilePieceIdx, bytes: Box<[u8]>) {
        use crate::{unwrap_or_return, IgnoreEmpty, OkOrLog};

        let has_piece = unwrap_or_return!(self.has_piece(piece_idx).await.ok_or_log());
        if !has_piece {
            self.try_set_piece(piece_idx, &bytes)
                .await
                .ok_or_log()
                .ignore_empty();
        }
    }

    pub async fn on_remote_pieces_received(
        self: &Arc<Self>,
        peer_id: PeerId,
        piece_idxs: Vec<FilePieceIdx>,
    ) {
        use crate::{IgnoreEmpty, OkOrLog};

        self.sharing_state
            .write()
            .await
            .set_peer_pieces_received(peer_id, piece_idxs)
            .ok_or_log()
            .ignore_empty();
    }

    pub async fn remove_remote_file(self: &Arc<Self>, peer_id: PeerId) {
        use crate::{IgnoreEmpty, OkOrLog};

        self.sharing_state
            .write()
            .await
            .remove_peer(peer_id)
            .ok_or_log()
            .ignore_empty();
    }

    pub async fn send_file_state(
        &self,
        peer: Arc<RemotePeer>,
    ) -> Result<(), FileSentFileStateError> {
        use crate::{now, PeerPeerMessage};
        use std::collections::hash_map::Entry;

        let peer_id = peer.peer_id();
        let mut status = self.remote_status.write().await;
        let status = status.entry(peer_id);
        let mut status = match status {
            Entry::Occupied(status) => Ok(status),
            Entry::Vacant(_) => Err(FileSentFileStateError::PeerIsNotAdded { peer_id }),
        }?;

        let state = self.sharing_state.read().await;
        let state = state.local_state();
        let sha256 = self.metadata.sha256();

        if state.is_missing() {
            peer.send(PeerPeerMessage::FileMissing { sha256 });
        } else if state.is_complete() {
            peer.send(PeerPeerMessage::FileComplete { sha256 });
        } else {
            peer.send(PeerPeerMessage::FileState {
                sha256,
                state: state.raw().to_owned().into_boxed_bitslice(),
            });
        }

        match status.get() {
            RemoteStateStatus::Received => {}
            RemoteStateStatus::Sent(_) | RemoteStateStatus::NotSent => {
                let now = now()?;
                let _ = status.insert(RemoteStateStatus::Sent(now));
            }
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Error, Debug)]
pub enum NewFileError {
    #[error("file size {len} is too large")]
    SizeIsTooLarge { len: FileLen },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileFromError {
    #[error("file size {len} is too large")]
    SizeIsTooLarge { len: FileLen },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileToBlobError {
    #[error(
        "file is not complete yet, \
         available pieces: {available}, \
         missing pieces: {missing}"
    )]
    NotComplete { available: usize, missing: usize },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileHasPieceError {
    #[error(transparent)]
    HasPieceError(#[from] FileStateHasPieceError),
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileGetPieceError {
    #[error(transparent)]
    HasPieceError(#[from] FileStateHasPieceError),
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileSetPieceError {
    #[error(transparent)]
    AddPieceError(#[from] FileSharingStateAddLocalPieceError),
    #[error("invalid piece {piece_idx} length {len}, expected {expected}")]
    InvalidPieceLen {
        piece_idx: FilePieceIdx,
        len: usize,
        expected: usize,
    },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileSentFileStateError {
    #[error("remote peer {peer_id} is not added to file")]
    PeerIsNotAdded { peer_id: PeerId },
    #[error(transparent)]
    NowError(#[from] NowError),
}
