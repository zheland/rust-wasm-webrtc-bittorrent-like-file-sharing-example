use core::cell::RefCell;
use std::sync::Arc;

use web_sys::Event;
use web_sys::{HtmlButtonElement, HtmlDivElement, HtmlInputElement};

use crate::{ClosureCell1, PeerParams, PeerUi};

#[derive(Debug)]
pub struct AppUi {
    peer_io: RefCell<Option<Arc<PeerUi>>>,
    app_div: HtmlDivElement,
    tracker_address_input: HtmlInputElement,
    upload_speed_limit_input: HtmlInputElement,
    max_channel_buffer_input: HtmlInputElement,
    peer_send_interval_input: HtmlInputElement,
    connect_button: HtmlButtonElement,
    upload_speed_handler: ClosureCell1<Event>,
    connect_click_handler: ClosureCell1<Event>,
}

impl AppUi {
    pub fn new() -> Arc<Self> {
        use crate::{body, ElementExt};
        use crate::{
            default_tracker_address, DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES,
            DEFAULT_PEER_SEND_INTERVAL_MS, DEFAULT_UPLOAD_SPEED_BITS_PER_SECOND,
        };

        let app_div: HtmlDivElement = body().unwrap().add_child("div").unwrap();

        let tracker_address_input = app_div
            .add_input("server address", &default_tracker_address())
            .unwrap();

        let upload_speed_limit_input = app_div
            .add_input(
                "upload speed limit (bits/s)",
                DEFAULT_UPLOAD_SPEED_BITS_PER_SECOND,
            )
            .unwrap();

        let max_channel_buffer_input = app_div
            .add_input(
                "max DataChannel buffer (bytes)",
                DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES,
            )
            .unwrap();

        let peer_send_interval_input = app_div
            .add_input("peer send interval (ms)", DEFAULT_PEER_SEND_INTERVAL_MS)
            .unwrap();

        let connect_button: HtmlButtonElement = body().unwrap().add_child("button").unwrap();
        connect_button.add_text("Connect to server").unwrap();

        let app = Arc::new(AppUi {
            peer_io: RefCell::new(None),
            app_div,
            tracker_address_input,
            upload_speed_limit_input,
            max_channel_buffer_input,
            peer_send_interval_input,
            connect_button,
            upload_speed_handler: RefCell::new(None),
            connect_click_handler: RefCell::new(None),
        });

        app.init();

        app
    }

    fn init(self: &Arc<Self>) {
        use crate::init_weak_callback;
        use web_sys::HtmlElement;

        init_weak_callback(
            &self,
            Self::on_connect_click,
            &self.connect_click_handler,
            HtmlElement::set_onclick,
            &self.connect_button,
        );
    }

    fn set_connect_buttons_inactive(&self) {
        self.tracker_address_input.set_read_only(true);
        self.upload_speed_limit_input.set_read_only(true);
        self.max_channel_buffer_input.set_read_only(true);
        self.peer_send_interval_input.set_read_only(true);
        self.connect_button.set_disabled(true);
    }

    fn on_connect_click(self: &Arc<Self>, _: Event) {
        use wasm_bindgen_futures::spawn_local;

        self.set_connect_buttons_inactive();
        let peer_params = PeerParams {
            tracker_addr: self.fix_and_get_tracker_address(),
            upload_speed_limit_bps: self.upload_speed_limit_input.value().parse().unwrap(),
            max_channel_buffer_bytes: self.max_channel_buffer_input.value().parse().unwrap(),
            peer_send_interval_ms: self.peer_send_interval_input.value().parse().unwrap(),
        };

        let self_arc = Arc::clone(self);
        spawn_local(async move {
            let peer_io = PeerUi::new(peer_params).await;
            let prev = self_arc.peer_io.replace(Some(peer_io));
            assert!(prev.is_none());
        });
    }

    fn fix_and_get_tracker_address(&self) -> String {
        let addr = self.tracker_address_input.value();
        if addr.starts_with("ws://") || addr.starts_with("wss://") {
            addr
        } else {
            let addr = format!("ws://{}", &addr);
            self.tracker_address_input.set_value(&addr);
            addr
        }
    }
}

impl Drop for AppUi {
    fn drop(&mut self) {
        self.connect_button.set_onclick(None);
        self.app_div.remove();
    }
}
