use js_sys::Uint8Array;
use thiserror::Error;
use tracker_protocol::FileSha256;
use web_sys::{Blob, File as WebSysFile};

use crate::{
    FileChunk, FileLen, FileMetadata, FilePieceIdx, FileState, FileStatePieceError,
    FileStateSetStatus, FILE_PIECE_SIZE,
};

// Chrome does not support creating an array buffer of 2 GB or more.
// Blob chunk size equal to 1MB is selected
// in order to optimize the process of file preparation for sharing
// and to reduce the probability of lags.
pub const FILE_CHUNK_SIZE: usize = 1048576;

const NUM_PIECES_IN_CHUNK: usize = FILE_CHUNK_SIZE / FILE_PIECE_SIZE;
static_assertions::const_assert_eq!(FILE_CHUNK_SIZE % FILE_PIECE_SIZE, 0);

pub type JsFile = File<Uint8Array, FILE_CHUNK_SIZE>;

#[derive(Debug)]
pub struct File<C, const CHUNK_SIZE: usize> {
    /// File metadata.
    metadata: FileMetadata,

    /// File contents separated into chunks.
    chunks: Vec<C>,

    /// Cached file pieces count.
    num_pieces: usize,

    /// Local file available pieces mask.
    state: FileState,
}

impl<C, const CHUNK_SIZE: usize> File<C, CHUNK_SIZE> {
    pub fn new(metadata: FileMetadata) -> Result<Self, NewFileError>
    where
        C: FileChunk,
    {
        use core::cmp::min;

        assert_eq!(CHUNK_SIZE as u64 % FILE_PIECE_SIZE_U64, 0);

        pub const FILE_PIECE_SIZE_U64: u64 = FILE_PIECE_SIZE as u64;
        pub const FILE_CHUNK_SIZE_U64: u64 = FILE_CHUNK_SIZE as u64;

        let len = metadata.len();
        let num_chunks: u64 = (len.0 + FILE_CHUNK_SIZE_U64 - 1) / FILE_CHUNK_SIZE_U64;
        let num_pieces = (len.0 + FILE_PIECE_SIZE_U64 - 1) / FILE_PIECE_SIZE_U64;
        let num_pieces: usize = num_pieces
            .try_into()
            .map_err(|_| NewFileError::SizeIsTooLarge { len })?;

        let chunks = (0..num_chunks)
            .map(|j| {
                let len = min(len.0 - j * FILE_CHUNK_SIZE_U64, FILE_CHUNK_SIZE_U64)
                    .try_into()
                    .unwrap();
                C::with_len(len)
            })
            .collect();

        let state = FileState::from_missing(num_pieces);

        Ok(Self {
            metadata,
            chunks,
            num_pieces,
            state,
        })
    }
}

impl<const CHUNK_SIZE: usize> File<Uint8Array, CHUNK_SIZE> {
    pub async fn from_file(file: WebSysFile) -> Result<Self, FileFromError> {
        use js_sys::{ArrayBuffer, Number};
        use sha2::{Digest, Sha256};
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;

        pub const FILE_PIECE_SIZE_U64: u64 = FILE_PIECE_SIZE as u64;
        pub const FILE_CHUNK_SIZE_U64: u64 = FILE_CHUNK_SIZE as u64;

        let name = file.name();
        let blob = file.slice().unwrap();
        let len_f64 = blob.size();
        let len = FileLen(len_f64 as u64);

        if !Number::is_safe_integer(&Number::from(len_f64)) {
            return Err(FileFromError::SizeIsTooLarge { len });
        }
        let num_pieces = (len.0 + FILE_PIECE_SIZE_U64 - 1) / FILE_PIECE_SIZE_U64;
        let num_pieces: usize = num_pieces
            .try_into()
            .map_err(|_| FileFromError::SizeIsTooLarge { len })?;

        let mut chunks = Vec::new();
        let mut hasher = Sha256::new();
        for start in (0..len.0).step_by(FILE_CHUNK_SIZE) {
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
            log::debug!("adding file {} ... {}/{}bytes", file.name(), start, len.0);
        }

        let sha256 = FileSha256(hasher.finalize().into());
        let metadata = FileMetadata::new(sha256, name, len);

        let state = FileState::from_complete(num_pieces);

        log::info!("adding file {} ... OK", file.name());
        Ok(Self {
            metadata,
            chunks,
            num_pieces,
            state,
        })
    }

    pub async fn to_blob(&self) -> Result<Blob, FileToBlobError> {
        use js_sys::Array;

        if self.state.is_complete() {
            let blob_args: Array = self.chunks.iter().collect();
            Ok(Blob::new_with_u8_array_sequence(&blob_args).unwrap())
        } else {
            Err(FileToBlobError::NotComplete {
                available: self.state.num_available(),
                missing: self.state.num_missing(),
            })
        }
    }
}

impl<C, const CHUNK_SIZE: usize> File<C, CHUNK_SIZE> {
    pub fn metadata(&self) -> &FileMetadata {
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

    pub fn state(&self) -> &FileState {
        &self.state
    }

    pub fn piece_len(&self, piece_idx: &FilePieceIdx) -> usize {
        let offset = piece_idx.0 * FILE_PIECE_SIZE;
        (self.len().0 - u64::try_from(offset).unwrap())
            .min(u64::try_from(FILE_PIECE_SIZE).unwrap())
            .try_into()
            .unwrap()
    }

    pub fn has_piece(&self, piece_idx: &FilePieceIdx) -> Result<bool, FileHasPieceError> {
        Ok(self.state.has(piece_idx)?)
    }

    pub fn get_piece(
        &self,
        piece_idx: &FilePieceIdx,
    ) -> Result<Option<Box<[u8]>>, FileGetPieceError>
    where
        C: FileChunk,
    {
        let chunk_idx = piece_idx.0 / NUM_PIECES_IN_CHUNK;
        let chunk_piece_idx = piece_idx.0 % NUM_PIECES_IN_CHUNK;

        let has_piece = self.state.has(piece_idx)?;
        if has_piece {
            let chunk = &self.chunks[chunk_idx];
            let offset = chunk_piece_idx * FILE_PIECE_SIZE;
            let len = self.piece_len(piece_idx);
            Ok(Some(chunk.get(offset, len)))
        } else {
            Ok(None)
        }
    }

    pub fn set_piece(
        &mut self,
        piece_idx: &FilePieceIdx,
        data: &[u8],
    ) -> Result<FileStateSetStatus, FileSetPieceError>
    where
        C: FileChunk,
    {
        let len = data.len();
        let expected = self.piece_len(piece_idx);

        if len == expected {
            let chunk_idx = piece_idx.0 / NUM_PIECES_IN_CHUNK;
            let chunk_piece_idx = piece_idx.0 % NUM_PIECES_IN_CHUNK;
            let chunk = &mut self.chunks[chunk_idx];
            let offset = chunk_piece_idx * FILE_PIECE_SIZE;

            chunk.set(offset, data);
            Ok(self.state.set(piece_idx)?)
        } else {
            Err(FileSetPieceError::InvalidPieceLen { expected })
        }
    }
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum NewFileError {
    #[error("file size {} is too large", len.0)]
    SizeIsTooLarge { len: FileLen },
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum FileFromError {
    #[error("file size {} is too large", len.0)]
    SizeIsTooLarge { len: FileLen },
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum FileToBlobError {
    #[error(
        "file is not complete yet, \
         available pieces: {available}, \
         missing pieces: {missing}"
    )]
    NotComplete { available: usize, missing: usize },
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum FileHasPieceError {
    #[error(transparent)]
    HasPieceError(#[from] FileStatePieceError),
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum FileGetPieceError {
    #[error(transparent)]
    HasPieceError(#[from] FileStatePieceError),
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum FileSetPieceError {
    #[error(transparent)]
    AddPieceError(#[from] FileStatePieceError),
    #[error("invalid piece length, expected: {expected}")]
    InvalidPieceLen { expected: usize },
}
