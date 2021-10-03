use core::cell::RefCell;
use std::sync::Arc;

use web_sys::Event;
use web_sys::{HtmlButtonElement, HtmlDivElement, HtmlInputElement};

use crate::{ClosureCell1, PeerUi};

#[derive(Debug)]
pub struct AppUi {
    peer: RefCell<Option<Arc<PeerUi>>>,
    app_div: HtmlDivElement,
    tracker_address_input: HtmlInputElement,
    connect_button: HtmlButtonElement,
    //upload_speed_handler: ClosureCell1<Event>,
    connect_click_handler: ClosureCell1<Event>,
}

impl AppUi {
    pub fn new() -> Arc<Self> {
        use crate::default_tracker_address;
        use crate::{body, ElementExt};

        let app_div: HtmlDivElement = body().unwrap().add_child("div").unwrap();

        let tracker_address_input = app_div
            .add_input("server address", &default_tracker_address())
            .unwrap();

        let connect_button: HtmlButtonElement = app_div.add_child("button").unwrap();
        connect_button.add_text("Connect to server").unwrap();

        let app = Arc::new(AppUi {
            peer: RefCell::new(None),
            app_div,
            tracker_address_input,
            //upload_speed_limit_input,
            //max_channel_buffer_input,
            //peer_send_interval_input,
            connect_button,
            //upload_speed_handler: RefCell::new(None),
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
        //self.upload_speed_limit_input.set_read_only(true);
        //self.max_channel_buffer_input.set_read_only(true);
        //self.peer_send_interval_input.set_read_only(true);
        self.connect_button.set_disabled(true);
    }

    fn on_connect_click(self: &Arc<Self>, _: Event) {
        use wasm_bindgen_futures::spawn_local;

        self.set_connect_buttons_inactive();
        let tracker_addr = self.fix_and_get_tracker_address();

        let self_arc = Arc::clone(self);
        spawn_local(async move {
            let peer = PeerUi::new(tracker_addr).await;
            let prev = self_arc.peer.replace(Some(peer));
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
