use std::sync::{Arc, Weak};

use thiserror::Error;
use tracker_protocol::{IceCandidate, PeerId, SdpType, SessionDescription};
use web_sys::{
    Event, MessageEvent, RtcConfiguration, RtcDataChannel, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcSdpType, RtcSessionDescriptionInit,
};

use crate::{ClosureCell1, Peer, PeerPeerMessage};

#[derive(Debug)]
pub struct Connection {
    peer: Weak<Peer>,
    other_peer_id: PeerId,
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
}

impl Connection {
    pub async fn new(peer: &Arc<Peer>, other_peer_id: PeerId) -> Arc<Self> {
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

        let connection = Arc::new(Self {
            peer: Arc::downgrade(peer),
            other_peer_id,
            peer_connection,
            data_channel,
            icecandidate_handler: RefCell::new(None),
            negotiationneeded_handler: RefCell::new(None),
            iceconnectionstatechange_handler: RefCell::new(None),
            icegatheringstatechange_handler: RefCell::new(None),
            signalingstatechange_handler: RefCell::new(None),
            data_message_handler: RefCell::new(None),
            data_open_handler: RefCell::new(None),
            data_error_handler: RefCell::new(None),
        });

        connection.init().await;

        connection
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

    pub async fn send_offer(&self) {
        use tracker_protocol::PeerTrackerMessage;
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;

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

        let peer = self.peer.upgrade().unwrap();
        let peer_id = self.other_peer_id;
        peer.send(PeerTrackerMessage::SendOffer { peer_id, offer });
    }

    async fn send_answer(&self) {
        use tracker_protocol::PeerTrackerMessage;
        use wasm_bindgen::JsCast;
        use wasm_bindgen::JsValue;
        use wasm_bindgen_futures::JsFuture;

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

        let peer = self.peer.upgrade().unwrap();
        let peer_id = self.other_peer_id;
        peer.send(PeerTrackerMessage::SendAnswer { peer_id, answer });
    }

    pub async fn on_peer_offer(self: &Arc<Self>, offer: SessionDescription) {
        use wasm_bindgen::JsValue;
        use wasm_bindgen_futures::JsFuture;

        log::debug!("remote offer: {:?}", offer);

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
        use tracker_protocol::PeerTrackerMessage;

        let peer = self.peer.upgrade().unwrap();
        let peer_id = self.other_peer_id;

        if let Some(candidate) = ev.candidate() {
            let candidate_str = candidate.candidate();
            match candidate_str.as_ref() {
                "" => {
                    log::debug!("local all ice candidates sent");
                    peer.send(PeerTrackerMessage::AllIceCandidatesSent { peer_id });
                }
                _ => {
                    let candidate = IceCandidate {
                        candidate: candidate_str,
                        sdp_mid: candidate.sdp_mid(),
                        sdp_mline_index: candidate.sdp_m_line_index(),
                        username_fragment: None,
                    };
                    log::debug!("local ice candidate: {:?}", candidate);
                    peer.send(PeerTrackerMessage::SendIceCandidate { peer_id, candidate });
                }
            };
        }
    }

    pub async fn on_peer_all_icecandidates_sent(self: &Arc<Self>) {
        log::debug!("remote all ice candidates sent");
    }

    fn on_negotiationneeded(self: &Arc<Self>, _: Event) {
        use wasm_bindgen_futures::spawn_local;

        let self_arc = Arc::clone(self);
        spawn_local(async move { self_arc.send_offer().await });
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

    pub fn send(
        self: &Arc<Self>,
        message: PeerPeerMessage,
        max_buffer_bytes: Option<u32>,
    ) -> Result<(), PeerConnectionSendError> {
        use bincode::serialize;

        // log::debug!("{:?}", message); // TODO: Remove
        if self.data_channel.buffered_amount() < max_buffer_bytes.unwrap_or(u32::MAX) {
            let request: Vec<u8> = serialize(&message).unwrap();
            self.data_channel.send_with_u8_array(&request).unwrap();
            Ok(())
        } else {
            Err(PeerConnectionSendError::BufferIsFilled)
        }
    }

    fn on_data_message(self: &Arc<Self>, ev: MessageEvent) {
        use bincode::deserialize;
        use js_sys::{ArrayBuffer, Uint8Array};
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let array_buffer: ArrayBuffer = ev.data().dyn_into().unwrap();
        let data = Uint8Array::new(&array_buffer).to_vec();
        let message = deserialize(&data).unwrap();
        // log::debug!("{:?}", message); // TODO: Remove

        let peer = self.peer.upgrade().unwrap();
        let other_peer_id = self.other_peer_id;
        spawn_local(async move {
            peer.on_peer_message(other_peer_id, message).await;
        });
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

#[derive(Error, Debug)]
pub enum PeerConnectionSendError {
    #[error("DataChannel buffer is filled")]
    BufferIsFilled,
}
