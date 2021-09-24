// https://stackoverflow.com/a/35697810
// The maximum safe UDP payload is 508 bytes
pub const CHUNK_SIZE: usize = 256;

pub const DEFAULT_UPLOAD_SPEED_BITS_PER_SECOND: &str = "65536";
pub const DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES: &str = "16777216";
pub const DEFAULT_PEER_SEND_INTERVAL_MS: &str = "1000";

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
