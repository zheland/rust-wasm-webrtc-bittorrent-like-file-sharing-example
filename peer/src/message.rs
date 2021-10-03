use bitvec::boxed::BitBox;
use serde::{Deserialize, Serialize};
use tracker_protocol::FileSha256;

use crate::FilePieceIdx;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum PeerPeerMessage {
    FileMissing {
        sha256: FileSha256,
    },
    FileComplete {
        sha256: FileSha256,
    },
    FileState {
        sha256: FileSha256,
        state: BitBox,
    },
    FileStateReceived {
        sha256: FileSha256,
    },
    FilePiece {
        sha256: FileSha256,
        piece_idx: FilePieceIdx,
        bytes: Box<[u8]>,
    },
    FilePiecesReceived {
        sha256: FileSha256,
        pieces: Vec<FilePieceIdx>,
    },
    FileRemoved {
        sha256: FileSha256,
    },
}
