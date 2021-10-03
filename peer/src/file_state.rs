use core::fmt::Debug;
use core::ops::BitAnd;

use bitvec::boxed::BitBox;
use bitvec::slice::BitSlice;
use thiserror::Error;

use crate::FilePieceIdx;

#[derive(Clone, Debug)]
pub struct FileState {
    raw: BitBox,
    num_available: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum FileStateSetStatus {
    JustSet,
    AlreadySet,
}

impl FileState {
    pub fn empty() -> Self {
        use bitvec::vec::BitVec;

        Self {
            raw: BitVec::new().into_boxed_bitslice(),
            num_available: 0,
        }
    }

    pub fn new_missing(len: usize) -> Self {
        use bitvec::bitbox;
        Self {
            raw: bitbox![0; len],
            num_available: 0,
        }
    }

    pub fn new_complete(len: usize) -> Self {
        use bitvec::bitbox;
        Self {
            raw: bitbox![1; len],
            num_available: len,
        }
    }

    pub fn from(pieces_mask: BitBox) -> Self {
        let num_available = pieces_mask.count_ones();
        Self {
            raw: pieces_mask,
            num_available,
        }
    }

    pub fn is_missing(&self) -> bool {
        self.num_available() == 0
    }

    pub fn is_complete(&self) -> bool {
        self.num_available() == self.len()
    }

    pub fn len(&self) -> usize {
        self.raw.len()
    }

    pub fn num_available(&self) -> usize {
        self.num_available
    }

    pub fn num_missing(&self) -> usize {
        self.len() - self.num_available()
    }

    pub fn raw(&self) -> &BitSlice {
        &self.raw
    }

    pub fn into_raw(self) -> BitBox {
        self.raw
    }

    pub fn has(&self, piece_idx: FilePieceIdx) -> Result<bool, FileStateHasPieceError> {
        let piece_idx_usize = usize::from(piece_idx);
        self.raw.get(piece_idx_usize).map_or_else(
            || {
                Err(FileStateHasPieceError::PieceIndexOutOfRange {
                    piece_idx,
                    len: self.len(),
                })
            },
            |value| Ok(*value),
        )
    }

    pub fn set(
        &mut self,
        piece_idx: FilePieceIdx,
    ) -> Result<FileStateSetStatus, FileStateSetPieceError> {
        let piece_idx_usize = usize::from(piece_idx);
        let len = self.len();
        let mask = self.raw.get_mut(piece_idx_usize);
        let mask = match mask {
            Some(mask) => mask,
            None => {
                return Err(FileStateSetPieceError::PieceIndexOutOfRange {
                    piece_idx,
                    len: len,
                })
            }
        };

        if *mask {
            Ok(FileStateSetStatus::AlreadySet)
        } else {
            mask.set(true);
            self.num_available += 1;
            Ok(FileStateSetStatus::JustSet)
        }
    }
}

impl BitAnd<&Self> for FileState {
    type Output = Self;

    fn bitand(self, rhs: &Self) -> Self {
        use bitvec::vec::BitVec;

        let mut state = self.raw.into_boxed_slice();
        for (lhs, rhs) in state.iter_mut().zip(rhs.raw.as_raw_slice()) {
            *lhs &= rhs;
        }
        let mask = BitVec::from_vec(state.into_vec()).into_boxed_bitslice();
        Self::from(mask)
    }
}

impl FileStateSetStatus {
    pub fn is_just_set(self) -> bool {
        match self {
            Self::JustSet => true,
            Self::AlreadySet => false,
        }
    }

    pub fn is_already_set(self) -> bool {
        match self {
            Self::JustSet => false,
            Self::AlreadySet => true,
        }
    }
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileStateHasPieceError {
    #[error("piece index {piece_idx} out of range for piece count {len}")]
    PieceIndexOutOfRange { piece_idx: FilePieceIdx, len: usize },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FileStateSetPieceError {
    #[error("piece index {piece_idx} out of range for piece count {len}")]
    PieceIndexOutOfRange { piece_idx: FilePieceIdx, len: usize },
}
