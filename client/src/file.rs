use bitvec::boxed::BitBox;
use bitvec::slice::BitSlice;
use thiserror::Error;
use tracker_protocol::Sha256;

use crate::CHUNK_SIZE;

#[derive(Debug)]
pub struct File {
    name: String,
    bytes: Box<[u8]>,
    chunks: BitBox,
}

impl File {
    pub fn new(name: String, len: usize) -> Self {
        use bitvec::bitbox;

        let chunks_len = (len + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let chunks = bitbox![0; chunks_len];
        let bytes = vec![0; len].into_boxed_slice();

        Self {
            name,
            bytes,
            chunks,
        }
    }

    pub fn from(name: String, bytes: Box<[u8]>) -> (Sha256, Self) {
        use bitvec::bitbox;
        use sha2::{Digest, Sha256};

        let chunks_len = (bytes.len() + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let chunks = bitbox![1; chunks_len];

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let sha256 = Sha256(hasher.finalize().into());

        (
            sha256,
            Self {
                name,
                bytes,
                chunks,
            },
        )
    }

    pub fn is_ready(&self) -> bool {
        self.chunks.all()
    }

    pub fn data(&self) -> &[u8] {
        &self.bytes
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn chunks(&self) -> &BitSlice {
        &self.chunks
    }

    pub fn chunks_len(&self) -> usize {
        self.chunks.len()
    }

    pub fn has_chunk(&self, chunk_idx: usize) -> Result<bool, FileHasChunkError> {
        self.chunks
            .get(chunk_idx)
            .map(|value| *value)
            .ok_or_else(|| FileHasChunkError::ChunkIndexOutOfRange {
                index: chunk_idx,
                len: self.chunks.len(),
            })
    }

    pub fn get_chunk(&self, chunk_idx: usize) -> Result<Option<&[u8]>, FileGetChunkError> {
        let chunk = self.chunks.get(chunk_idx).map(|value| *value);

        match chunk {
            Some(true) => {
                let offset = chunk_idx * CHUNK_SIZE;
                if chunk_idx < self.chunks.len() - 1 {
                    Ok(Some(&self.bytes[offset..offset + CHUNK_SIZE]))
                } else {
                    Ok(Some(&self.bytes[offset..]))
                }
            }
            Some(false) => Ok(None),
            None => Err(FileGetChunkError::ChunkIndexOutOfRange {
                index: chunk_idx,
                len: self.chunks.len(),
            }),
        }
    }

    pub fn set_chunk(&mut self, chunk_idx: usize, bytes: &[u8]) -> Result<(), FileSetChunkError> {
        let chunks_len = self.chunks.len();
        let chunks = self.chunks.get_mut(chunk_idx);
        let chunks = match chunks {
            Some(chunks) => chunks,
            None => {
                return Err(FileSetChunkError::ChunkIndexOutOfRange {
                    index: chunk_idx,
                    len: chunks_len,
                })
            }
        };

        let chunk_len = if chunk_idx < chunks_len - 1 {
            if bytes.len() != CHUNK_SIZE {
                return Err(FileSetChunkError::InvalidChunkLen {
                    len: self.bytes.len(),
                });
            }
            CHUNK_SIZE
        } else {
            let last_chunk_len = self.bytes.len() - CHUNK_SIZE * chunk_idx;
            if bytes.len() != last_chunk_len {
                return Err(FileSetChunkError::InvalidLastChunkLen {
                    len: self.bytes.len(),
                    expected: last_chunk_len,
                });
            }
            last_chunk_len
        };
        if *chunks {
            return Err(FileSetChunkError::ChunkIsAlreadySet { index: chunk_idx });
        }

        let offset = chunk_idx * CHUNK_SIZE;
        chunks.set(true);
        self.bytes[offset..offset + chunk_len].copy_from_slice(bytes);
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum FileHasChunkError {
    #[error("chunk index {index} out of range for chunk count {len}")]
    ChunkIndexOutOfRange { index: usize, len: usize },
}

#[derive(Error, Debug)]
pub enum FileGetChunkError {
    #[error("chunk index {index} out of range for chunk count {len}")]
    ChunkIndexOutOfRange { index: usize, len: usize },
}

#[derive(Error, Debug)]
pub enum FileSetChunkError {
    #[error("chunk index {index} out of range for chunk count {len}")]
    ChunkIndexOutOfRange { index: usize, len: usize },
    #[error("invalid chunk length {len}, expected {}", CHUNK_SIZE)]
    InvalidChunkLen { len: usize },
    #[error("invalid last chunk length {len}, expected {expected}")]
    InvalidLastChunkLen { len: usize, expected: usize },
    #[error("chunk {index} is already set")]
    ChunkIsAlreadySet { index: usize },
}
