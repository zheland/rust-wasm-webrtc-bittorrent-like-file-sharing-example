use core::fmt::Debug;
use core::ops::BitAnd;

use bitvec::boxed::BitBox;
use bitvec::ptr::{BitRef, Const, Mut};
use bitvec::slice::BitSlice;
use thiserror::Error;

use crate::FilePieceIdx;

#[derive(Clone, Debug)]
pub struct FileState {
    raw: BitBox,
    num_available: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileStateSetStatus {
    JustSet,
    AlreadySet,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileStateUnsetStatus {
    JustUnset,
    AlreadyUnset,
}

impl FileState {
    pub fn empty() -> Self {
        use bitvec::vec::BitVec;

        Self {
            raw: BitVec::new().into_boxed_bitslice(),
            num_available: 0,
        }
    }

    pub fn from_missing(len: usize) -> Self {
        use bitvec::bitbox;
        Self {
            raw: bitbox![0; len],
            num_available: 0,
        }
    }

    pub fn from_complete(len: usize) -> Self {
        use bitvec::bitbox;
        Self {
            raw: bitbox![1; len],
            num_available: len,
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

    pub fn num_missing(&self) -> usize {
        self.len() - self.num_available()
    }

    pub fn num_available(&self) -> usize {
        self.num_available
    }

    pub fn raw(&self) -> &BitSlice {
        &self.raw
    }

    pub fn into_raw(self) -> BitBox {
        self.raw
    }

    fn get<'a>(
        &'a self,
        piece_idx: &FilePieceIdx,
    ) -> Result<BitRef<'a, Const>, FileStatePieceError> {
        self.raw
            .get(piece_idx.0)
            .ok_or_else(|| FileStatePieceError::PieceIndexOutOfRange)
    }

    fn get_mut<'a>(
        &'a mut self,
        piece_idx: &FilePieceIdx,
    ) -> Result<BitRef<'a, Mut>, FileStatePieceError> {
        self.raw
            .get_mut(piece_idx.0)
            .ok_or_else(|| FileStatePieceError::PieceIndexOutOfRange)
    }

    pub fn has(&self, piece_idx: &FilePieceIdx) -> Result<bool, FileStatePieceError> {
        self.get(piece_idx).map(|ok| *ok)
    }

    pub fn set(
        &mut self,
        piece_idx: &FilePieceIdx,
    ) -> Result<FileStateSetStatus, FileStatePieceError> {
        let bit = self.get_mut(piece_idx)?;

        if *bit {
            Ok(FileStateSetStatus::AlreadySet)
        } else {
            bit.set(true);
            self.num_available += 1;
            Ok(FileStateSetStatus::JustSet)
        }
    }

    pub fn unset(
        &mut self,
        piece_idx: &FilePieceIdx,
    ) -> Result<FileStateUnsetStatus, FileStatePieceError> {
        let bit = self.get_mut(piece_idx)?;

        if *bit {
            bit.set(false);
            self.num_available -= 1;
            Ok(FileStateUnsetStatus::JustUnset)
        } else {
            Ok(FileStateUnsetStatus::AlreadyUnset)
        }
    }
}

impl From<BitBox> for FileState {
    fn from(pieces_mask: BitBox) -> Self {
        let num_available = pieces_mask.count_ones();
        Self {
            raw: pieces_mask,
            num_available,
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

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum FileStatePieceError {
    #[error("piece index out of range")]
    PieceIndexOutOfRange,
}
