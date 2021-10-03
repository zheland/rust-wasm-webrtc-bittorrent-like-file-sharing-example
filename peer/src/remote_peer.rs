use core::sync::atomic::AtomicBool;
use std::sync::{Arc, Weak};

use thiserror::Error;
use tracker_protocol::{IceCandidate, PeerId, SdpType, SessionDescription};
use web_sys::{
    Event, MessageEvent, RtcConfiguration, RtcDataChannel, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcSdpType, RtcSessionDescriptionInit,
};

use crate::{ClosureCell1, LocalPeer, PeerPeerMessage};

#[derive(Clone, Copy, Debug)]
pub enum RemotePeerKind {
    Offering,
    Answering,
}

#[derive(Debug)]
pub enum RemotePeerState {
    Offering,
    Answering { has_offer: AtomicBool },
}

// TODO: Remove remote peer on peer connection dropped
#[derive(Debug)]
pub struct RemotePeer {
    local_peer: Weak<LocalPeer>,
    peer_id: PeerId,
    state: RemotePeerState,
    peer_connection: RtcPeerConnection,
    data_channel: RtcDataChannel,
    icecandidate_handler: ClosureCell1<RtcPeerConnectionIceEvent>,
    negotiationneeded_handler: ClosureCell1<Event>,
    iceconnectionstatechange_handler: ClosureCell1<Event>,
    icegatheringstatechange_handler: ClosureCell1<Event>,
    signalingstatechange_handler: ClosureCell1<Event>,
    data_message_handler: ClosureCell1<MessageEvent>,
    data_open_handler: ClosureCell1<Event>,
    data_error_handler: ClosureCell1<Event>,
    //files: RwLock<HashMap<Sha256, Weak<RemoteFile>>>,
}

impl RemotePeer {
    pub async fn new(
        local_peer: &Arc<LocalPeer>,
        peer_id: PeerId,
        kind: RemotePeerKind,
    ) -> Arc<Self> {
        use core::cell::RefCell;
        use web_sys::{RtcDataChannelInit, RtcDataChannelType};

        let peer_connection =
            RtcPeerConnection::new_with_configuration(&default_rtc_configuration()).unwrap();
        let mut data_channel_init = RtcDataChannelInit::new();
        let _: &mut _ = data_channel_init.id(0);
        let _: &mut _ = data_channel_init.negotiated(true);
        let _: &mut _ = data_channel_init.ordered(false);
        let _: &mut _ = data_channel_init.max_retransmits(0);
        let data_channel =
            peer_connection.create_data_channel_with_data_channel_dict("data", &data_channel_init);
        data_channel.set_binary_type(RtcDataChannelType::Arraybuffer);
        let state = match kind {
            RemotePeerKind::Offering => RemotePeerState::Offering,
            RemotePeerKind::Answering => RemotePeerState::Answering {
                has_offer: AtomicBool::new(false),
            },
        };

        let remote_peer = Arc::new(Self {
            local_peer: Arc::downgrade(local_peer),
            peer_id,
            peer_connection,
            data_channel,
            state,
            icecandidate_handler: RefCell::new(None),
            negotiationneeded_handler: RefCell::new(None),
            iceconnectionstatechange_handler: RefCell::new(None),
            icegatheringstatechange_handler: RefCell::new(None),
            signalingstatechange_handler: RefCell::new(None),
            data_message_handler: RefCell::new(None),
            data_open_handler: RefCell::new(None),
            data_error_handler: RefCell::new(None),
            //files: RwLock::new(HashMap::new()),
        });

        remote_peer.init().await;

        remote_peer
    }

    async fn init(self: &Arc<Self>) {
        use crate::init_weak_callback;

        init_weak_callback(
            &self,
            Self::on_icecandidate,
            &self.icecandidate_handler,
            RtcPeerConnection::set_onicecandidate,
            &self.peer_connection,
        );

        init_weak_callback(
            &self,
            Self::on_negotiationneeded,
            &self.negotiationneeded_handler,
            RtcPeerConnection::set_onnegotiationneeded,
            &self.peer_connection,
        );

        init_weak_callback(
            &self,
            Self::on_iceconnectionstatechange,
            &self.iceconnectionstatechange_handler,
            RtcPeerConnection::set_oniceconnectionstatechange,
            &self.peer_connection,
        );

        init_weak_callback(
            &self,
            Self::on_icegatheringstatechange,
            &self.icegatheringstatechange_handler,
            RtcPeerConnection::set_onicegatheringstatechange,
            &self.peer_connection,
        );

        init_weak_callback(
            &self,
            Self::on_signalingstatechange,
            &self.signalingstatechange_handler,
            RtcPeerConnection::set_onsignalingstatechange,
            &self.peer_connection,
        );

        init_weak_callback(
            &self,
            Self::on_data_message,
            &self.data_message_handler,
            RtcDataChannel::set_onmessage,
            &self.data_channel,
        );

        init_weak_callback(
            &self,
            Self::on_data_open,
            &self.data_open_handler,
            RtcDataChannel::set_onopen,
            &self.data_channel,
        );

        init_weak_callback(
            &self,
            Self::on_data_error,
            &self.data_error_handler,
            RtcDataChannel::set_onerror,
            &self.data_channel,
        );
    }

    async fn send_offer(&self) {
        use crate::unwrap_or_return;
        use tracker_protocol::PeerTrackerMessage;
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;

        let local_peer = unwrap_or_return!(self.local_peer.upgrade());

        let offer = JsFuture::from(self.peer_connection.create_offer())
            .await
            .unwrap();
        let offer: &RtcSessionDescriptionInit = offer.as_ref().unchecked_ref();

        let _: JsValue = JsFuture::from(self.peer_connection.set_local_description(offer))
            .await
            .unwrap();

        let offer = SessionDescription {
            sdp_type: offer.get_sdp_type().unwrap(),
            sdp: offer.get_sdp().unwrap(),
        };
        log::debug!("local offer: {:?}", offer);

        let peer_id = self.peer_id;
        local_peer.send(PeerTrackerMessage::SendOffer { peer_id, offer });
    }

    async fn send_answer(&self) {
        use crate::unwrap_or_return;
        use tracker_protocol::PeerTrackerMessage;
        use wasm_bindgen::JsCast;
        use wasm_bindgen::JsValue;
        use wasm_bindgen_futures::JsFuture;

        let local_peer = unwrap_or_return!(self.local_peer.upgrade());

        let answer = JsFuture::from(self.peer_connection.create_answer())
            .await
            .unwrap();
        let answer: &RtcSessionDescriptionInit = answer.as_ref().unchecked_ref();

        let _: JsValue = JsFuture::from(self.peer_connection.set_local_description(answer))
            .await
            .unwrap();

        let answer = SessionDescription {
            sdp_type: answer.get_sdp_type().unwrap(),
            sdp: answer.get_sdp().unwrap(),
        };
        log::debug!("local answer: {:?}", answer);

        let peer_id = self.peer_id;
        local_peer.send(PeerTrackerMessage::SendAnswer { peer_id, answer });
    }

    pub async fn on_peer_offer(self: &Arc<Self>, offer: SessionDescription) {
        use std::sync::atomic::Ordering;
        use wasm_bindgen::JsValue;
        use wasm_bindgen_futures::JsFuture;

        log::debug!("remote offer: {:?}", offer);

        match &self.state {
            RemotePeerState::Offering => {
                // TODO: Maybe return result
                log::error!("offer received by the offering peer");
            }
            RemotePeerState::Answering { has_offer } => has_offer.store(true, Ordering::Relaxed),
        }

        let sdp_type = protocol_sdp_type_to_web_sys_sdp_type(offer.sdp_type);
        let mut remote_description = RtcSessionDescriptionInit::new(sdp_type);
        let _: &mut _ = remote_description.sdp(&offer.sdp);

        let peer_connection = self.peer_connection.clone();
        let _: JsValue =
            JsFuture::from(peer_connection.set_remote_description(&remote_description))
                .await
                .unwrap();

        self.send_answer().await;
    }

    pub async fn on_peer_answer(self: &Arc<Self>, answer: SessionDescription) {
        use wasm_bindgen::JsValue;
        use wasm_bindgen_futures::JsFuture;

        log::debug!("remote answer: {:?}", answer);

        match &self.state {
            RemotePeerState::Offering => {}
            RemotePeerState::Answering { .. } => {
                // TODO: Maybe return result
                log::error!("answer received by the answering peer");
                return;
            }
        }

        let sdp_type = protocol_sdp_type_to_web_sys_sdp_type(answer.sdp_type);
        let mut remote_description = RtcSessionDescriptionInit::new(sdp_type);
        let _: &mut _ = remote_description.sdp(&answer.sdp);

        let peer_connection = self.peer_connection.clone();
        let _: JsValue =
            JsFuture::from(peer_connection.set_remote_description(&remote_description))
                .await
                .unwrap();
    }

    pub async fn on_peer_icecandidate(self: &Arc<Self>, candidate: IceCandidate) {
        use wasm_bindgen::JsValue;
        use wasm_bindgen_futures::JsFuture;
        use web_sys::{RtcIceCandidate, RtcIceCandidateInit};

        log::debug!("remote ice candidate: {:?}", candidate);

        let mut candidate_init = RtcIceCandidateInit::new(&candidate.candidate);
        let _: &mut _ = candidate_init
            .sdp_mid(candidate.sdp_mid.as_deref())
            .sdp_m_line_index(candidate.sdp_mline_index);
        let candidate = RtcIceCandidate::new(&candidate_init).unwrap();

        let _: JsValue = JsFuture::from(
            self.peer_connection
                .add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate)),
        )
        .await
        .unwrap();
    }

    fn on_icecandidate(self: &Arc<Self>, ev: RtcPeerConnectionIceEvent) {
        use crate::unwrap_or_return;
        use tracker_protocol::PeerTrackerMessage;

        let local_peer = unwrap_or_return!(self.local_peer.upgrade());
        let candidate = unwrap_or_return!(ev.candidate());
        let peer_id = self.peer_id;

        let candidate_str = candidate.candidate();
        match candidate_str.as_ref() {
            "" => {
                log::debug!("local all ice candidates sent");
                local_peer.send(PeerTrackerMessage::AllIceCandidatesSent { peer_id });
            }
            _ => {
                let candidate = IceCandidate {
                    candidate: candidate_str,
                    sdp_mid: candidate.sdp_mid(),
                    sdp_mline_index: candidate.sdp_m_line_index(),
                    username_fragment: None,
                };
                log::debug!("local ice candidate: {:?}", candidate);
                local_peer.send(PeerTrackerMessage::SendIceCandidate { peer_id, candidate });
            }
        };
    }

    pub async fn on_peer_all_icecandidates_sent(self: &Arc<Self>) {
        log::debug!("remote all ice candidates sent");
    }

    fn on_negotiationneeded(self: &Arc<Self>, _: Event) {
        use core::sync::atomic::Ordering;
        use wasm_bindgen_futures::spawn_local;

        let self_arc = Arc::clone(self);
        // TODO: Do not send offer if send in progress
        match &self.state {
            RemotePeerState::Offering => spawn_local(async move { self_arc.send_offer().await }),
            RemotePeerState::Answering { has_offer } => {
                if has_offer.load(Ordering::Relaxed) {
                    spawn_local(async move { self_arc.send_answer().await })
                }
            }
        };
    }

    fn on_iceconnectionstatechange(self: &Arc<Self>, _: Event) {
        log::debug!(
            "ice connection state: {:?}",
            self.peer_connection.ice_connection_state()
        );
    }

    fn on_icegatheringstatechange(self: &Arc<Self>, _: Event) {
        log::debug!(
            "ice gathering state: {:?}",
            self.peer_connection.ice_gathering_state()
        );
    }

    fn on_signalingstatechange(self: &Arc<Self>, _: Event) {
        log::debug!(
            "signaling state: {:?}",
            self.peer_connection.signaling_state()
        );
    }

    pub fn is_ready(&self) -> bool {
        use web_sys::RtcDataChannelState;

        self.data_channel.ready_state() == RtcDataChannelState::Open
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn send(&self, message: PeerPeerMessage) {
        use crate::FilePieceIdx;
        use tracker_protocol::FileSha256;

        match &message {
            PeerPeerMessage::FilePiece {
                sha256,
                piece_idx,
                bytes: _,
            } => {
                #[derive(Clone, Debug)]
                struct FilePiece<'a> {
                    sha256: &'a FileSha256,
                    piece_idx: &'a FilePieceIdx,
                }
                log::trace!(
                    "recv peer_message {} {:?}",
                    self.peer_id,
                    FilePiece { sha256, piece_idx }
                ); // TODO: Remove
            }
            message => {
                log::trace!("recv peer_message {} {:?}", self.peer_id, message);
                // TODO: Remove
            }
        }
        log::trace!("send peer_message {} {:?}", self.peer_id, message); // TODO: Remove

        use bincode::serialize;
        let request: Vec<u8> = serialize(&message).unwrap();
        self.data_channel.send_with_u8_array(&request).unwrap();
    }

    pub fn send_with_max_buffer_size(
        &self,
        message: PeerPeerMessage,
        max_buffer_bytes: u64,
    ) -> Result<(), PeerConnectionSendError> {
        use bincode::serialize;

        if (self.data_channel.buffered_amount() as u64) < max_buffer_bytes {
            let request: Vec<u8> = serialize(&message).unwrap();
            self.data_channel.send_with_u8_array(&request).unwrap();
            Ok(())
        } else {
            Err(PeerConnectionSendError::BufferIsFilled)
        }
    }

    fn on_data_open(self: &Arc<Self>, _: Event) {
        log::debug!("data channel opened");
    }

    fn on_data_error(self: &Arc<Self>, ev: Event) {
        use js_sys::Reflect;
        use wasm_bindgen::JsValue;

        let error = Reflect::get(&ev, &JsValue::from_str("error")).unwrap();
        log::error!("data channel error: {:?}", error);
    }

    fn on_data_message(self: &Arc<Self>, ev: MessageEvent) {
        use crate::unwrap_or_return;
        use bincode::deserialize;
        use js_sys::{ArrayBuffer, Uint8Array};
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let local_peer = unwrap_or_return!(self.local_peer.upgrade());

        let array_buffer: ArrayBuffer = ev.data().dyn_into().unwrap();
        let data = Uint8Array::new(&array_buffer).to_vec();
        let message = deserialize(&data).unwrap();

        let remote_peer = Arc::clone(self);
        spawn_local(async move {
            local_peer.on_peer_message(&remote_peer, message).await;
        });
    }
}

fn default_rtc_configuration() -> RtcConfiguration {
    use js_sys::Array;
    use wasm_bindgen::JsValue;
    use web_sys::RtcIceServer;

    let mut configuration = RtcConfiguration::new();

    let ice_server_urls = vec![JsValue::from("stun:stun.l.google.com:19302")];
    let ice_server_urls: Array = ice_server_urls.into_iter().collect();
    let mut ice_server = RtcIceServer::new();
    let _: &mut _ = ice_server.urls(&JsValue::from(ice_server_urls));

    let ice_servers: Array = vec![ice_server].into_iter().collect();
    let _: &mut _ = configuration.ice_servers(&JsValue::from(ice_servers));

    configuration
}

fn sdp_type_to_protocol_sdp_type(sdp_type: &str) -> Option<SdpType> {
    match sdp_type {
        "offer" => Some(SdpType::Offer),
        "answer" => Some(SdpType::Answer),
        "pranswer" => Some(SdpType::Pranswer),
        "rollback" => Some(SdpType::Rollback),
        _ => None,
    }
}

fn protocol_sdp_type_to_web_sys_sdp_type(sdp_type: SdpType) -> RtcSdpType {
    match sdp_type {
        SdpType::Offer => RtcSdpType::Offer,
        SdpType::Answer => RtcSdpType::Answer,
        SdpType::Pranswer => RtcSdpType::Pranswer,
        SdpType::Rollback => RtcSdpType::Rollback,
    }
}

trait RtcSessionDescriptionInitExt {
    fn get_sdp(&self) -> Option<String>;
    fn get_sdp_type(&self) -> Option<SdpType>;
}

impl RtcSessionDescriptionInitExt for RtcSessionDescriptionInit {
    fn get_sdp(&self) -> Option<String> {
        use js_sys::Reflect;
        use wasm_bindgen::JsValue;

        Reflect::get(&self, &JsValue::from_str("sdp"))
            .ok()
            .and_then(|value| value.as_string())
    }

    fn get_sdp_type(&self) -> Option<SdpType> {
        use js_sys::Reflect;
        use wasm_bindgen::JsValue;

        let sdp_type = Reflect::get(&self, &JsValue::from_str("type"))
            .unwrap()
            .as_string()
            .unwrap();
        sdp_type_to_protocol_sdp_type(&sdp_type)
    }
}

#[derive(Clone, Copy, Error, Debug)]
pub enum PeerConnectionSendError {
    #[error("DataChannel buffer is filled")]
    BufferIsFilled,
}
