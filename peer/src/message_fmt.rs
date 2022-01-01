use core::borrow::Borrow;
use core::fmt::{self, Display};

use tracker_protocol::FileSha256;

use crate::PeerPeerMessage;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PeerPeerMessageFmt<T>(pub T);

fn short_sha_hex(sha256: &FileSha256) -> String {
    hex::encode_upper(&sha256.0[0..4])
}

impl<T> Display for PeerPeerMessageFmt<T>
where
    T: Borrow<PeerPeerMessage>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0.borrow() {
            PeerPeerMessage::FileMissing { sha256 } => {
                write!(f, "{}: file missing", short_sha_hex(sha256))
            }
            PeerPeerMessage::FileComplete { sha256 } => {
                write!(f, "{}: file complete", short_sha_hex(sha256))
            }
            PeerPeerMessage::FileState { sha256, state } => {
                let state: String = state
                    .iter()
                    .map(|bit| if *bit { '+' } else { '-' })
                    .collect();
                write!(f, "{}: file state: {}", short_sha_hex(sha256), state)
            }
            PeerPeerMessage::FileStateReceived { sha256 } => {
                write!(f, "{}: file state received", short_sha_hex(sha256))
            }
            PeerPeerMessage::FilePiece {
                sha256,
                piece_idx,
                bytes,
            } => {
                write!(
                    f,
                    "{}: file piece {} with bytes of length {}",
                    short_sha_hex(sha256),
                    piece_idx.0,
                    bytes.len()
                )
            }
            PeerPeerMessage::FilePiecesReceived { sha256, pieces } => {
                let pieces: Vec<_> = pieces.iter().map(|piece| piece.0).collect();
                write!(
                    f,
                    "{}: file pieces received: {:?}",
                    short_sha_hex(sha256),
                    pieces
                )
            }
            PeerPeerMessage::FileRemoved { sha256 } => {
                write!(f, "{}: file removed", short_sha_hex(sha256))
            }
        }
    }
}
