#![warn(
    clippy::all,
    rust_2018_idioms,
    missing_copy_implementations,
    missing_debug_implementations,
    single_use_lifetimes,
    trivial_casts,
    unused_import_braces,
    unused_qualifications,
    unused_results
)]

mod file_meta_data;
mod file_piece;
mod file_pieces_queues;
mod file_sharing_selector;
mod file_sharing_state;
mod file_state;
mod local_file;
mod local_peer;
mod message;
mod object_url;
mod params;
mod peer_sender;
mod remote_peer;
mod scheduler;
mod tracker;

mod callback;
mod ignore_empty;
mod interval_handler;
mod js_rand;
mod js_time;
mod ok_or_log;
mod upwrap_or;
mod vec_ext;

pub use file_meta_data::{FileLen, FileMetaData};
pub use file_piece::{
    FilePieceData, FilePieceIdx, PieceNumOwners, PiecePeerShift, PieceSendAttempts, FILE_PIECE_SIZE,
};
pub use file_pieces_queues::{
    FilePiecesQueueAddError, FilePiecesQueueRemoveError, FilePiecesQueues,
};
pub use file_sharing_selector::FileSharingSelector;
pub use file_sharing_state::{
    FileSharingState, FileSharingStateAddLocalPieceError, FileSharingStateAddPeerError,
    FileSharingStateRemovePeerError,
};
pub use file_state::{
    FileState, FileStateHasPieceError, FileStateSetPieceError, FileStateSetStatus,
};
pub use local_file::{
    FileGetPieceError, FileSentFileStateError, FileSetPieceError, LocalFile, RemoteStateStatus,
};
pub use local_peer::LocalPeer;
pub use message::PeerPeerMessage;
pub use object_url::ObjectUrl;
pub use params::{
    DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES, DEFAULT_PEER_SEND_INTERVAL_MS,
    DEFAULT_UPLOAD_SPEED_BITS_PER_SECOND,
};
pub use peer_sender::{PeerSender, PeerSenderParams};
pub use remote_peer::{PeerConnectionSendError, RemotePeer, RemotePeerKind};
pub use scheduler::macrotask;
pub use tracker::Tracker;

pub use callback::{init_weak_callback, Callback, ClosureCell0, ClosureCell1};
use ignore_empty::IgnoreEmpty;
use interval_handler::{IntervalHandler, NewIntervalHandlerError};
use js_rand::JsRandom;
use js_time::{now, NowError};
use ok_or_log::OkOrLog;
use vec_ext::{PushAndReturnOffset, SetWithResizeDefault};
