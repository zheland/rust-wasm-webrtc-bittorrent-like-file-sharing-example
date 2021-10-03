use derive_more::{Display, From, Into};
use serde::{Deserialize, Serialize};

// The maximum UDP package length: 65535 bytes
// IPv4 minimum reassembly buffer size: 576 bytes (ignored)
// Ethernet MTU: ~1500 bytes
pub const FILE_PIECE_SIZE: usize = 1024;

#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Display,
    Eq,
    From,
    Hash,
    Into,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
pub struct FilePieceIdx(pub u32);

pub type PiecePeerShift = u32;
pub type PieceNumOwners = u32;
pub type PieceSendAttempts = u32;

impl From<usize> for FilePieceIdx {
    fn from(value: usize) -> Self {
        Self(value.try_into().unwrap())
    }
}

impl From<FilePieceIdx> for usize {
    fn from(value: FilePieceIdx) -> Self {
        value.0.try_into().unwrap()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FilePieceData {
    pub idx: FilePieceIdx,
    pub peer_shift: PiecePeerShift,
    pub num_owners: PieceNumOwners,
    pub send_attempts: PieceSendAttempts,
}
