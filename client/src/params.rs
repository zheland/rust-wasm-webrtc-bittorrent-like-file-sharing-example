pub const DEFAULT_UPLOAD_SPEED_BYTES_PER_SECOND: &str = "1048576";
pub const DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES: &str = "2097152";
pub const DEFAULT_PEER_DATA_SEND_INTERVAL: &str = "0.1";
pub const DEFAULT_STATE_RESEND_INTERVAL: &str = "10";
pub const DEFAULT_PIECE_RESEND_INTERVAL: &str = "0.5";

pub fn default_tracker_address() -> String {
    const FALLBACK_ADDRESS: &str = "ws://localhost:9010";

    use js_sys::{JsString, Reflect};
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::window;

    window()
        .and_then(|window| Reflect::get(&window, &JsValue::from_str("tracker_address")).ok())
        .and_then(|addr| addr.dyn_into().ok())
        .map(|addr: JsString| addr.into())
        .unwrap_or(FALLBACK_ADDRESS.to_owned())
}
