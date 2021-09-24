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

use core::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum SdpType {
    Offer,
    Answer,
    Pranswer,
    Rollback,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SessionDescription {
    pub sdp_type: SdpType,
    pub sdp: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u16>,
    pub username_fragment: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Sha256(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PeerId(pub u32);

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum PeerTrackerMessage {
    RequestOffers {
        file_sha256: Sha256,
    },
    SendOffer {
        peer_id: PeerId,
        offer: SessionDescription,
    },
    SendAnswer {
        peer_id: PeerId,
        answer: SessionDescription,
    },
    SendIceCandidate {
        peer_id: PeerId,
        candidate: IceCandidate,
    },
    AllIceCandidatesSent {
        peer_id: PeerId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum TrackerPeerMessage {
    PeerIdAssigned {
        peer_id: PeerId,
    },
    RequestOffer {
        peer_id: PeerId,
        file_sha256: Sha256,
    },
    PeerOffer {
        peer_id: PeerId,
        offer: SessionDescription,
    },
    PeerAnswer {
        peer_id: PeerId,
        answer: SessionDescription,
    },
    PeerIceCandidate {
        peer_id: PeerId,
        candidate: IceCandidate,
    },
    PeerAllIceCandidatesSent {
        peer_id: PeerId,
    },
}

impl fmt::Display for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode_upper(self.0))
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
