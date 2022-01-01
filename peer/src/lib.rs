#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    rust_2018_idioms,
    missing_copy_implementations,
    missing_debug_implementations,
    single_use_lifetimes,
    trivial_casts,
    unused_import_braces,
    unused_qualifications,
    unused_results
)]

mod file;
mod file_chunk;
mod file_metadata;
mod file_piece;
mod file_pieces_queues;
mod file_state;
mod local_peer;
mod message;
mod message_fmt;
mod object_url;
mod params;
mod remote_peer;
mod scheduler;
mod shared_file;
mod tracker;

mod callback;
mod ignore_empty;
mod ok_or_log;
mod upwrap_or;
mod vec_ext;

pub use file::{
    File, FileGetPieceError, FileHasPieceError, FileSetPieceError, JsFile, FILE_CHUNK_SIZE,
};
pub use file_chunk::FileChunk;
pub use file_metadata::{FileLen, FileMetadata};
pub use file_piece::{
    FilePieceData, FilePieceIdx, PieceNumConfirmedOwners, PieceNumPossibleOwners, PiecePeerShift,
    FILE_PIECE_SIZE,
};
pub use file_pieces_queues::{
    FilePiecesQueueGetError, FilePiecesQueueInsertError, FilePiecesQueueRemoveError,
    FilePiecesQueues,
};
pub use file_state::{FileState, FileStatePieceError, FileStateSetStatus, FileStateUnsetStatus};
pub use local_peer::LocalPeer;
pub use message::PeerPeerMessage;
pub use message_fmt::PeerPeerMessageFmt;
pub use object_url::ObjectUrl;
pub use params::{
    DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES, DEFAULT_PEER_SEND_INTERVAL_MS,
    DEFAULT_UPLOAD_SPEED_BITS_PER_SECOND,
};
pub use remote_peer::{PeerConnectionSendError, RemotePeer, RemotePeerKind};
pub use scheduler::macrotask;
pub use shared_file::{
    JsSharedFile, LocalStateStatusError, SharedFile, SharedFileAddLocalPieceError,
    SharedFileAddPeerError, SharedFileLocalStateStatus, SharedFileMarkStatus,
    SharedFileRemovePeerError,
};
pub use tracker::Tracker;

pub use callback::{init_weak_callback, Callback, ClosureCell0, ClosureCell1};
use ignore_empty::IgnoreEmpty;
use ok_or_log::OkOrLog;
use vec_ext::{PushAndReturnOffset, SetWithResizeDefault};
