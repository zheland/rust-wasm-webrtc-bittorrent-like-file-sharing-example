use tracker_protocol::{PeerTrackerMessage, TrackerPeerMessage};
use web_sys::{MessageEvent, WebSocket};

use crate::ClosureCell1;

#[derive(Debug)]
pub struct Tracker {
    websocket: WebSocket,
    message_handler: ClosureCell1<MessageEvent>,
}

impl Tracker {
    pub async fn new(tracker_addr: String) -> Self {
        use core::cell::RefCell;
        use js_sys::Promise;
        use wasm_bindgen::JsValue;
        use wasm_bindgen_futures::JsFuture;
        use web_sys::BinaryType;

        let websocket = WebSocket::new(tracker_addr.as_ref()).unwrap();
        websocket.set_binary_type(BinaryType::Arraybuffer);

        let web_socket_opened = Promise::new(&mut |resolve, reject| {
            websocket.set_onopen(Some(&resolve));
            websocket.set_onerror(Some(&reject));
        });
        let _: JsValue = JsFuture::from(web_socket_opened).await.unwrap();

        let message_handler = RefCell::new(None);

        Self {
            websocket,
            message_handler,
        }
    }

    pub fn set_handler<F: 'static + FnMut(TrackerPeerMessage)>(&self, mut callback: F) {
        use crate::Callback;
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        let closure = Closure::with_callback(move |ev| {
            let message = Self::parse(&ev);
            callback(message)
        });
        self.websocket
            .set_onmessage(Some(closure.as_ref().unchecked_ref()));
        let _: Option<_> = self.message_handler.replace(Some(closure));
    }

    pub fn send(&self, message: PeerTrackerMessage) {
        use bincode::serialize;

        let request: Vec<u8> = serialize(&message).unwrap();
        self.websocket.send_with_u8_array(&request).unwrap();
    }

    fn parse(message: &MessageEvent) -> TrackerPeerMessage {
        use bincode::deserialize;
        use js_sys::{ArrayBuffer, Uint8Array};
        use wasm_bindgen::JsCast;

        let array_buffer: ArrayBuffer = message.data().dyn_into().unwrap();
        let data = Uint8Array::new(&array_buffer).to_vec();
        let message = deserialize(&data).unwrap();
        log::debug!("{:?}", message);
        message
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        use crate::IgnoreEmpty;

        self.websocket.set_onmessage(None);
        self.websocket.close().ok().ignore_empty();
    }
}
