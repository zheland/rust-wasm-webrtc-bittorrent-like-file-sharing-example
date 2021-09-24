use bitvec::boxed::BitBox;
use serde::{Deserialize, Serialize};
use tracker_protocol::Sha256;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum PeerPeerMessage {
    FileMetaData {
        sha256: Sha256,
        name: String,
        len: usize,
    },
    FileState {
        sha256: Sha256,
        chunks: BitBox,
    },
    FileChunk {
        sha256: Sha256,
        chunk_idx: usize,
        bytes: Box<[u8]>,
    },
}
