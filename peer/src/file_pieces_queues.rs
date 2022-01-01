use nonmax::NonMaxUsize;
use thiserror::Error;

use crate::{FilePieceData, FilePieceIdx, PieceNumPossibleOwners};

type PeerOffset = NonMaxUsize;

#[derive(Clone, Debug)]
pub struct FilePiecesQueues {
    sharable_pieces: Vec<Option<FilePiecesQueuePiece>>,
    pieces_by_num_possible_owners: Vec<Vec<FilePieceIdx>>,
    min_possible_owners: Option<PieceNumPossibleOwners>,
}

#[derive(Clone, Copy, Debug)]
pub struct FilePiecesQueuePiece {
    offset: PeerOffset,
    data: FilePieceData,
}

static_assertions::const_assert_eq!(
    core::mem::size_of::<FilePiecesQueuePiece>(),
    core::mem::size_of::<Option<FilePiecesQueuePiece>>()
);

impl FilePiecesQueues {
    pub fn new(num_pieces: usize) -> Self {
        Self {
            sharable_pieces: vec![None; num_pieces],
            pieces_by_num_possible_owners: Vec::new(),
            min_possible_owners: None,
        }
    }

    pub fn next_queue(&self) -> Option<(PieceNumPossibleOwners, &[FilePieceIdx])> {
        match self.min_possible_owners {
            Some(idx) => Some((idx, &self.pieces_by_num_possible_owners[idx.0])),
            None => None,
        }
    }

    /// Gets piece data from piece queues.
    ///
    /// Returns piece data for available pieces not yet received by all receivers.
    pub fn get(
        &mut self,
        piece_idx: FilePieceIdx,
    ) -> Result<FilePieceData, FilePiecesQueueGetError> {
        match &self.sharable_pieces.get(piece_idx.0) {
            Some(Some(piece)) => Ok(piece.data),
            Some(None) => Err(FilePiecesQueueGetError::PieceIsNotAdded),
            None => Err(FilePiecesQueueGetError::PieceIndexOutOfRange {
                len: self.sharable_pieces.len(),
            }),
        }
    }

    /// Inserts piece data into piece queues.
    ///
    /// Should be used when piece is available but not yet received by all receivers.
    pub fn insert(
        &mut self,
        piece_idx: FilePieceIdx,
        data: FilePieceData,
    ) -> Result<(), FilePiecesQueueInsertError> {
        use crate::{PushAndReturnOffset, SetWithResizeDefault};

        match self.sharable_pieces.get(piece_idx.0) {
            Some(Some(_)) => Err(FilePiecesQueueInsertError::PieceIsAlreadyAdded),
            Some(None) => {
                let pieces = self
                    .pieces_by_num_possible_owners
                    .get_mut_or_resize_default(data.num_possible_owners.0);
                let offset = NonMaxUsize::new(pieces.push_and_get_offset(piece_idx)).unwrap();
                self.min_possible_owners = Some(match self.min_possible_owners {
                    None => data.num_possible_owners,
                    Some(value) => value.min(data.num_possible_owners),
                });
                self.sharable_pieces[piece_idx.0] = Some(FilePiecesQueuePiece { offset, data });
                Ok(())
            }
            None => Err(FilePiecesQueueInsertError::PieceIndexOutOfRange {
                len: self.sharable_pieces.len(),
            }),
        }
    }

    /// Removes piece data from piece queues.
    ///
    /// Should be used when to schange piece data or after it has been received by all receivers.
    pub fn remove(
        &mut self,
        piece_idx: &FilePieceIdx,
    ) -> Result<FilePieceData, FilePiecesQueueRemoveError> {
        match self.sharable_pieces.get_mut(piece_idx.0).map(Option::take) {
            Some(Some(piece)) => {
                let offset = piece.offset.get();
                let pieces =
                    &mut self.pieces_by_num_possible_owners[piece.data.num_possible_owners.0];
                let stored_piece_idx = pieces.swap_remove(offset);
                debug_assert_eq!(piece_idx, &stored_piece_idx);

                if offset != pieces.len() {
                    let moved_piece_idx = pieces[offset];
                    self.sharable_pieces[moved_piece_idx.0]
                        .as_mut()
                        .unwrap()
                        .offset = offset.try_into().unwrap();
                }

                self.update_min_possible_owners_after_remove();
                Ok(piece.data)
            }
            Some(None) => Err(FilePiecesQueueRemoveError::PieceIsNotAdded),
            None => Err(FilePiecesQueueRemoveError::PieceIndexOutOfRange {
                len: self.sharable_pieces.len(),
            }),
        }
    }

    fn update_min_possible_owners_after_remove(&mut self) {
        for list_idx in
            self.min_possible_owners.unwrap().0..self.pieces_by_num_possible_owners.len()
        {
            if !self.pieces_by_num_possible_owners[list_idx].is_empty() {
                self.min_possible_owners = Some(PieceNumPossibleOwners(list_idx));
                return;
            }
        }
        self.min_possible_owners = None;
    }
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum FilePiecesQueueGetError {
    #[error("piece index out of range for piece count {len}")]
    PieceIndexOutOfRange { len: usize },
    #[error("piece is not added to FilePiecesQueue")]
    PieceIsNotAdded,
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum FilePiecesQueueInsertError {
    #[error("piece index out of range for piece count {len}")]
    PieceIndexOutOfRange { len: usize },
    #[error("piece is already added to FilePiecesQueue")]
    PieceIsAlreadyAdded,
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum FilePiecesQueueRemoveError {
    #[error("piece index out of range for piece count {len}")]
    PieceIndexOutOfRange { len: usize },
    #[error("piece is not added to FilePiecesQueue")]
    PieceIsNotAdded,
}
