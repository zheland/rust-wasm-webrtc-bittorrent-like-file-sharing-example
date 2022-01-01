use core::cell::RefCell;
use std::sync::Arc;

use async_std::sync::RwLock;
use peer::LocalPeer;
use web_sys::{Event, HtmlButtonElement, HtmlDivElement, HtmlInputElement};

use crate::{
    ClosureCell1, FileUi, Sender, SenderParams, Time, DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES,
    DEFAULT_PEER_DATA_SEND_INTERVAL, DEFAULT_PIECE_RESEND_INTERVAL, DEFAULT_STATE_RESEND_INTERVAL,
    DEFAULT_UPLOAD_SPEED_BYTES_PER_SECOND,
};

#[derive(Debug)]
pub struct PeerUi {
    local_peer: Arc<LocalPeer<Time>>,
    local_files: RwLock<Vec<Arc<FileUi>>>,
    peer_sender: RwLock<Option<Sender>>,
    peer_div: HtmlDivElement,
    recv_div: HtmlDivElement,
    send_div: HtmlDivElement,
    file_input: HtmlInputElement,
    magnet_input: HtmlInputElement,
    recv_button: HtmlButtonElement,
    send_button: HtmlButtonElement,
    upload_speed_limit_input: HtmlInputElement,
    max_channel_buffer_input: HtmlInputElement,
    peer_send_interval_input: HtmlInputElement,
    state_resend_interval_input: HtmlInputElement,
    piece_resend_interval_input: HtmlInputElement,
    file_input_handler: ClosureCell1<Event>,
    recv_button_handler: ClosureCell1<Event>,
    send_button_handler: ClosureCell1<Event>,
    upload_speed_limit_handler: ClosureCell1<Event>,
    max_channel_buffer_handler: ClosureCell1<Event>,
    peer_send_interval_handler: ClosureCell1<Event>,
    state_resend_interval_handler: ClosureCell1<Event>,
    piece_resend_interval_handler: ClosureCell1<Event>,
}

impl PeerUi {
    pub async fn new(tracker_addr: String) -> Arc<Self> {
        use crate::{body, ElementExt};

        let peer_div: HtmlDivElement = body().unwrap().add_div().unwrap();

        let local_peer = LocalPeer::new(tracker_addr).await;

        peer_div.add_div().unwrap().add_text("Peer:").unwrap();

        let upload_speed_limit_input = peer_div
            .add_div()
            .unwrap()
            .add_input(
                "upload speed limit (bytes/s):",
                DEFAULT_UPLOAD_SPEED_BYTES_PER_SECOND,
            )
            .unwrap();

        let max_channel_buffer_input = peer_div
            .add_div()
            .unwrap()
            .add_input(
                "max DataChannel buffer (bytes):",
                DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES,
            )
            .unwrap();

        let peer_send_interval_input = peer_div
            .add_div()
            .unwrap()
            .add_input(
                "peer data send interval (seconds):",
                DEFAULT_PEER_DATA_SEND_INTERVAL,
            )
            .unwrap();

        let state_resend_interval_input = peer_div
            .add_div()
            .unwrap()
            .add_input(
                "state resend interval (seconds):",
                DEFAULT_STATE_RESEND_INTERVAL,
            )
            .unwrap();

        let piece_resend_interval_input = peer_div
            .add_div()
            .unwrap()
            .add_input(
                "piece resend interval (seconds):",
                DEFAULT_PIECE_RESEND_INTERVAL,
            )
            .unwrap();

        let recv_div: HtmlDivElement = peer_div.add_div().unwrap();
        let send_div: HtmlDivElement = peer_div.add_div().unwrap();

        recv_div.add_div().unwrap().add_text("Receive:").unwrap();
        let magnet_input: HtmlInputElement = recv_div.add_input("magnet", "").unwrap();
        magnet_input.class_list().add_1("magnet").unwrap();

        let recv_button: HtmlButtonElement = recv_div.add_child("button").unwrap();
        recv_button.add_text("Receive file by magnet").unwrap();

        send_div.add_div().unwrap().add_text("Send:").unwrap();
        let file_input: HtmlInputElement = send_div.add_child("input").unwrap();
        file_input.set_type("file");
        file_input.class_list().add_1("fileinput").unwrap();

        let send_button: HtmlButtonElement = send_div.add_child("button").unwrap();
        send_button.set_disabled(true);
        send_button.add_text("Send file").unwrap();

        let peer_ui = Arc::new(Self {
            local_peer,
            local_files: RwLock::new(Vec::new()),
            peer_sender: RwLock::new(None),
            peer_div,
            recv_div,
            send_div,
            file_input,
            magnet_input,
            recv_button,
            send_button,
            upload_speed_limit_input,
            max_channel_buffer_input,
            peer_send_interval_input,
            state_resend_interval_input,
            piece_resend_interval_input,
            //peer_sender_handler: RefCell::new(None),
            file_input_handler: RefCell::new(None),
            recv_button_handler: RefCell::new(None),
            send_button_handler: RefCell::new(None),
            upload_speed_limit_handler: RefCell::new(None),
            max_channel_buffer_handler: RefCell::new(None),
            peer_send_interval_handler: RefCell::new(None),
            state_resend_interval_handler: RefCell::new(None),
            piece_resend_interval_handler: RefCell::new(None),
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

        init_weak_callback(
            &self,
            Self::on_send_click,
            &self.send_button_handler,
            HtmlElement::set_onclick,
            &self.send_button,
        );

        init_weak_callback(
            &self,
            Self::on_update_peer_sender,
            &self.upload_speed_limit_handler,
            HtmlElement::set_onchange,
            &self.upload_speed_limit_input,
        );

        init_weak_callback(
            &self,
            Self::on_update_peer_sender,
            &self.max_channel_buffer_handler,
            HtmlElement::set_onchange,
            &self.max_channel_buffer_input,
        );

        init_weak_callback(
            &self,
            Self::on_update_peer_sender,
            &self.peer_send_interval_handler,
            HtmlElement::set_onchange,
            &self.peer_send_interval_input,
        );

        init_weak_callback(
            &self,
            Self::on_update_peer_sender,
            &self.state_resend_interval_handler,
            HtmlElement::set_onchange,
            &self.state_resend_interval_input,
        );

        init_weak_callback(
            &self,
            Self::on_update_peer_sender,
            &self.piece_resend_interval_handler,
            HtmlElement::set_onchange,
            &self.piece_resend_interval_input,
        );

        self.update_peer_sender();
    }

    fn on_update_peer_sender(self: &Arc<Self>, _: Event) {
        self.update_peer_sender();
    }

    fn update_peer_sender(self: &Arc<Self>) {
        use peer::FILE_PIECE_SIZE;
        use std::time::Duration;
        use wasm_bindgen_futures::spawn_local;

        let upload_speed_limit: Result<u64, _> = self.upload_speed_limit_input.value().parse();
        let max_channel_buffer: Result<u64, _> = self.max_channel_buffer_input.value().parse();
        let peer_send_interval: Result<f64, _> = self.peer_send_interval_input.value().parse();
        let state_resend_interval: Result<f64, _> =
            self.state_resend_interval_input.value().parse();
        let piece_resend_interval: Result<f64, _> =
            self.piece_resend_interval_input.value().parse();

        let (
            upload_speed_limit,
            max_channel_buffer,
            peer_send_interval,
            state_resend_interval,
            piece_resend_interval,
        ) = match (
            upload_speed_limit,
            max_channel_buffer,
            peer_send_interval,
            state_resend_interval,
            piece_resend_interval,
        ) {
            (Ok(v1), Ok(v2), Ok(v3), Ok(v4), Ok(v5)) => (v1, v2, v3, v4, v5),
            (v1, v2, v3, v4, v5) => {
                log::error!(
                    "PeerSender params parse failed: {:?} {:?} {:?} {:?} {:?}",
                    v1,
                    v2,
                    v3,
                    v4,
                    v5
                );
                return;
            }
        };

        let peer_ui = Arc::clone(&self);

        let update_callback = move || {
            let peer_ui = Arc::clone(&peer_ui);
            spawn_local(async move {
                for file_ui in peer_ui.local_files.read().await.iter() {
                    file_ui.update().await;
                }
            })
        };

        let peer_ui = Arc::clone(&self);
        spawn_local(async move {
            let _: Option<_> = peer_ui.peer_sender.write().await.replace(
                Sender::new(
                    Arc::clone(&peer_ui.local_peer),
                    SenderParams {
                        data_send_interval: Duration::from_secs_f64(peer_send_interval),
                        state_resend_interval: Duration::from_secs_f64(state_resend_interval),
                        piece_resend_interval: Duration::from_secs_f64(piece_resend_interval),
                        num_pieces_to_be_sent: ((upload_speed_limit as f64 * peer_send_interval)
                            as u64
                            / FILE_PIECE_SIZE as u64)
                            as usize,
                        max_buffer_bytes: Some(max_channel_buffer),
                    },
                    update_callback,
                )
                .unwrap(),
            );
        });
    }

    fn on_recv_click(self: &Arc<Self>, _: Event) {
        use peer::{File, FileMetadata};
        use wasm_bindgen_futures::spawn_local;

        let magnet = self.magnet_input.value();
        let magnet = magnet.trim();
        let metadata = FileMetadata::decode_base64(magnet);
        let metadata = match metadata {
            Ok(metadata) => metadata,
            Err(err) => {
                log::error!("error on magnet decode {}", err);
                return;
            }
        };

        let peer_ui = Arc::clone(&self);
        spawn_local(async move {
            let file = File::new(metadata);
            match file {
                Ok(file) => {
                    let shared_file = peer_ui.local_peer.add_file(file).await.unwrap();
                    let file_ui = FileUi::new(shared_file).await;
                    peer_ui.local_files.write().await.push(file_ui);
                }
                Err(err) => {
                    log::error!("LocalFile::new error: {}", err);
                }
            };
        });
    }

    fn on_file_input(self: &Arc<Self>, _: Event) {
        let num_files = self.file_input.files().map_or(0, |files| files.length());

        if num_files == 0 {
            self.send_button.set_disabled(true);
        } else {
            self.send_button.set_disabled(false);
        }
    }

    fn on_send_click(self: &Arc<Self>, _: Event) {
        use peer::File;
        use wasm_bindgen_futures::spawn_local;

        let files = self.file_input.files();
        if let Some(files) = files {
            for j in 0..files.length() {
                let file = files.get(j).unwrap();
                let peer_ui = Arc::clone(&self);
                spawn_local(async move {
                    let file = File::from_file(file).await;
                    match file {
                        Ok(file) => {
                            let shared_file = peer_ui.local_peer.add_file(file).await.unwrap();
                            let file_ui = FileUi::new(shared_file).await;
                            peer_ui.local_files.write().await.push(file_ui);
                        }
                        Err(err) => {
                            log::error!("LocalFile::from_file error: {}", err);
                        }
                    };
                });
            }
        }
        self.file_input.set_value("");
        self.send_button.set_disabled(true);
    }
}

impl Drop for PeerUi {
    fn drop(&mut self) {
        let _ = self.recv_div;
        let _ = self.send_div;
        self.peer_div.remove();
    }
}
