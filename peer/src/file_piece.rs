use serde::{Deserialize, Serialize};

// The maximum UDP package length: 65535 bytes
// IPv4 minimum reassembly buffer size: 576 bytes (ignored)
// Ethernet MTU: ~1500 bytes
pub const FILE_PIECE_SIZE: usize = 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FilePieceIdx(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PiecePeerShift(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PieceNumConfirmedOwners(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PieceNumPossibleOwners(pub usize);

#[derive(Clone, Copy, Debug)]
pub struct FilePieceData {
    pub peer_shift: PiecePeerShift,
    pub num_confirmed_owners: PieceNumConfirmedOwners,
    pub num_possible_owners: PieceNumPossibleOwners,
}
