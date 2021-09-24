use core::cell::RefCell;
use std::sync::Arc;

use web_sys::{Event, HtmlButtonElement, HtmlDivElement, HtmlInputElement};

use crate::{ClosureCell1, Peer, PeerParams};

#[derive(Debug)]
pub struct PeerUi {
    peer: Arc<Peer>,
    peer_div: HtmlDivElement,
    file_input: HtmlInputElement,
    sha256_input: HtmlInputElement,
    recv_button: HtmlButtonElement,
    file_input_handler: ClosureCell1<Event>,
    recv_button_handler: ClosureCell1<Event>,
}

impl PeerUi {
    pub async fn new(params: PeerParams) -> Arc<Self> {
        use crate::{body, ElementExt};

        let peer_div: HtmlDivElement = body().unwrap().add_child("div").unwrap();

        let peer = Peer::new(params).await;

        let sha256_input: HtmlInputElement = peer_div.add_input("sha256", "").unwrap();
        sha256_input.class_list().add_1("sha256").unwrap();

        let recv_button: HtmlButtonElement = peer_div.add_child("button").unwrap();
        recv_button.add_text("Receive file by sha256").unwrap();

        let file_input: HtmlInputElement = peer_div.add_child("input").unwrap();
        file_input.set_type("file");
        file_input.class_list().add_1("fileinput").unwrap();

        let peer_ui = Arc::new(Self {
            peer,
            peer_div,
            file_input,
            sha256_input,
            recv_button,
            file_input_handler: RefCell::new(None),
            recv_button_handler: RefCell::new(None),
        });

        peer_ui.init();

        peer_ui
    }

    fn init(self: &Arc<Self>) {
        use crate::init_weak_callback;
        use web_sys::HtmlElement;

        init_weak_callback(
            &self,
            Self::on_file_input,
            &self.file_input_handler,
            HtmlElement::set_onchange,
            &self.file_input,
        );

        init_weak_callback(
            &self,
            Self::on_recv_click,
            &self.recv_button_handler,
            HtmlElement::set_onclick,
            &self.recv_button,
        );
    }

    fn on_file_input(self: &Arc<Self>, _: Event) {
        use crate::html::ElementExt;
        use js_sys::Uint8Array;
        use wasm_bindgen::JsValue;
        use wasm_bindgen_futures::spawn_local;
        use wasm_bindgen_futures::JsFuture;

        let files = self.file_input.files();
        if let Some(files) = files {
            for j in 0..files.length() {
                let file = files.get(j).unwrap();
                let name = file.name();
                let array_buffer = file.array_buffer();
                let peer = Arc::clone(&self.peer);
                let peer_div = self.peer_div.clone();
                spawn_local(async move {
                    let array_buffer: JsValue = JsFuture::from(array_buffer).await.unwrap();
                    let array = Uint8Array::new(&array_buffer);
                    let bytes = array.to_vec().into_boxed_slice();
                    log::info!("share file `{}`", name);
                    let sha256 = peer.share_file(name.clone(), bytes).await;
                    if let Some(sha256) = sha256 {
                        let input: HtmlInputElement =
                            peer_div.add_input(&name, &format!("{}", sha256)).unwrap();
                        input.class_list().add_1("sha256").unwrap();
                        input.set_read_only(true);
                    }
                });
            }
        }
        self.file_input.set_value("");
    }

    fn on_recv_click(self: &Arc<Self>, _: Event) {
        use hex::decode;
        use std::convert::TryInto;
        use tracker_protocol::Sha256;
        use wasm_bindgen_futures::spawn_local;

        let sha256 = self.sha256_input.value();
        let sha256 = sha256.trim();
        let sha256 = match decode(sha256) {
            Ok(sha256) => sha256,
            Err(err) => {
                log::error!("File SHA256 decode error {}", err);
                return;
            }
        };

        let sha256: [u8; 32] = match sha256.try_into() {
            Ok(sha256) => sha256,
            Err(vec) => {
                log::error!("Invalid sha256 bit length {}", vec.len() * 8);
                return;
            }
        };

        let sha256 = Sha256(sha256);
        let peer = Arc::clone(&self.peer);
        spawn_local(async move {
            log::info!("receive file `{}`", sha256);
            peer.load_file(sha256).await;
        });
    }
}

impl Drop for PeerUi {
    fn drop(&mut self) {
        self.peer_div.remove();
    }
}
