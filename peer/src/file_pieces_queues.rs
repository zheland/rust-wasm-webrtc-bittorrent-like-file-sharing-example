use thiserror::Error;

use crate::{FilePieceData, FilePieceIdx, PieceNumOwners, PiecePeerShift, PieceSendAttempts};

type PeerOffset = nonmax::NonMaxU32;

#[derive(Clone, Debug)]
pub struct FilePiecesQueues {
    pieces: Vec<FilePiecesQueuePiece>,
    // The first queue vec element contain the vec of the rerest pieces.
    // The last queue vec element contain the vec of the most frequent pieces.
    queues: Vec<Vec<FilePieceIdx>>,
    next_queue_idx: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FilePiecesQueuePiece {
    num_owners: PieceNumOwners,
    peer_shift: PiecePeerShift,
    send_attempts: PieceSendAttempts,
    offset: Option<PeerOffset>,
}

impl FilePiecesQueues {
    pub fn new(num_pieces: usize) -> Self {
        Self {
            pieces: vec![FilePiecesQueuePiece::default(); num_pieces],
            queues: Vec::new(),
            next_queue_idx: None,
        }
    }

    pub fn next_queue(&self) -> Option<(usize, &[FilePieceIdx])> {
        match self.next_queue_idx {
            Some(idx) => Some((idx, &self.queues[idx])),
            None => None,
        }
    }

    pub fn add(&mut self, data: FilePieceData) -> Result<(), FilePiecesQueueAddError> {
        use crate::PushAndReturnOffset;
        use crate::SetWithResizeDefault;
        use nonmax::NonMaxU32;

        let num_pieces = self.pieces.len();
        let piece = self.pieces.get_mut(usize::from(data.idx)).ok_or_else(|| {
            FilePiecesQueueAddError::PieceIndexOutOfRange {
                piece_idx: data.idx,
                len: num_pieces,
            }
        })?;
        if piece.offset.is_some() {
            return Err(FilePiecesQueueAddError::PieceIsAlreadyAdded {
                piece_idx: data.idx,
            });
        }

        piece.num_owners = data.num_owners;
        piece.send_attempts = data.send_attempts;
        let list_idx = FilePiecesQueuePiece {
            num_owners: data.num_owners,
            peer_shift: data.peer_shift,
            send_attempts: data.send_attempts,
            offset: None,
        }
        .list_idx();
        self.next_queue_idx = Some(self.next_queue_idx.unwrap_or(list_idx).min(list_idx));

        let pieces = self.queues.get_mut_or_resize_default(list_idx);
        let offset = pieces.push_and_get_offset(data.idx);
        let offset = u32::try_from(offset).unwrap();
        let offset = NonMaxU32::new(offset).unwrap();

        let _ = piece.offset.replace(offset);
        Ok(())
    }

    pub fn get(
        &self,
        piece_idx: FilePieceIdx,
    ) -> Result<&FilePiecesQueuePiece, FilePiecesQueueGetError> {
        let piece = self.pieces.get(usize::from(piece_idx)).ok_or_else(|| {
            FilePiecesQueueGetError::PieceIndexOutOfRange {
                piece_idx,
                len: self.pieces.len(),
            }
        })?;
        if piece.offset.is_some() {
            Ok(piece)
        } else {
            Err(FilePiecesQueueGetError::PieceIsNotAdded { piece_idx })
        }
    }

    pub fn remove(
        &mut self,
        piece_idx: FilePieceIdx,
    ) -> Result<FilePieceData, FilePiecesQueueRemoveError> {
        use crate::SetWithResizeDefault;

        let num_pieces = self.pieces.len();
        let piece = self.pieces.get_mut(usize::from(piece_idx)).ok_or_else(|| {
            FilePiecesQueueRemoveError::PieceIndexOutOfRange {
                piece_idx,
                len: num_pieces,
            }
        })?;
        if piece.offset.is_none() {
            return Err(FilePiecesQueueRemoveError::PieceIsNotAdded { piece_idx });
        }

        let list_idx = piece.list_idx();

        let offset_nm = piece.offset.take().unwrap();
        let offset = usize::try_from(offset_nm.get()).unwrap();
        let list_pieces = self.queues.get_mut_or_resize_default(list_idx);
        let _ = list_pieces.swap_remove(offset);

        let piece = *piece;
        if let Some(&moved_piece_idx) = list_pieces.get(offset) {
            self.pieces[usize::from(moved_piece_idx)].offset = Some(offset_nm);
        }

        if list_idx == self.next_queue_idx.unwrap() {
            'outer: loop {
                for list_idx in list_idx..self.queues.len() {
                    if !self.queues[list_idx].is_empty() {
                        self.next_queue_idx = Some(list_idx);
                        break 'outer;
                    }
                }
                self.next_queue_idx = None;
                break;
            }
        }

        Ok(FilePieceData {
            idx: piece_idx,
            peer_shift: piece.peer_shift,
            num_owners: piece.num_owners,
            send_attempts: piece.send_attempts,
        })
    }
}

impl FilePiecesQueuePiece {
    pub fn peer_shift(&self) -> PiecePeerShift {
        self.peer_shift
    }

    pub fn num_owners(&self) -> PieceNumOwners {
        self.num_owners
    }

    pub fn send_attempts(&self) -> PieceSendAttempts {
        self.send_attempts
    }

    pub fn list_idx(&self) -> usize {
        const MAX_SEND_ATTEMPTS_SHIFT: u32 = 256;

        usize::try_from(self.num_owners).unwrap()
            + usize::try_from(self.send_attempts.min(MAX_SEND_ATTEMPTS_SHIFT)).unwrap()
    }
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FilePiecesQueueAddError {
    #[error("piece index {piece_idx} out of range for piece count {len}")]
    PieceIndexOutOfRange { piece_idx: FilePieceIdx, len: usize },
    #[error("piece {piece_idx} is already added to FilePiecesQueue")]
    PieceIsAlreadyAdded { piece_idx: FilePieceIdx },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FilePiecesQueueGetError {
    #[error("piece index {piece_idx} out of range for piece count {len}")]
    PieceIndexOutOfRange { piece_idx: FilePieceIdx, len: usize },
    #[error("piece {piece_idx} is not added to FilePiecesQueue")]
    PieceIsNotAdded { piece_idx: FilePieceIdx },
}

#[derive(Clone, Copy, Error, Debug)]
pub enum FilePiecesQueueRemoveError {
    #[error("piece index {piece_idx} out of range for piece count {len}")]
    PieceIndexOutOfRange { piece_idx: FilePieceIdx, len: usize },
    #[error("piece {piece_idx} is not added to FilePiecesQueue")]
    PieceIsNotAdded { piece_idx: FilePieceIdx },
}
