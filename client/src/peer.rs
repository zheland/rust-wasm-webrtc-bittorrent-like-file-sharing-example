use core::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use async_std::sync::RwLock;
use bitvec::boxed::BitBox;
use tracker_protocol::{PeerId, PeerTrackerMessage, Sha256, TrackerPeerMessage};

use crate::{ClosureCell0, Connection, File, PeerPeerMessage, Tracker};

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct PeerParams {
    pub tracker_addr: String,
    pub upload_speed_limit_bps: u32,
    pub max_channel_buffer_bytes: u32,
    pub peer_send_interval_ms: u32,
}

#[derive(Debug)]
pub struct Peer {
    params: PeerParams,
    tracker: Tracker,
    peer_id: RefCell<Option<PeerId>>,
    peer_connections: RwLock<HashMap<PeerId, Arc<Connection>>>,
    files: RwLock<HashMap<Sha256, Arc<RwLock<File>>>>,
    files_peer_state: RwLock<HashMap<Sha256, Arc<RwLock<HashMap<PeerId, Option<BitBox>>>>>>,
    interval_id: RefCell<Option<i32>>,
    update_handler: ClosureCell0,
}

impl Peer {
    pub async fn new(params: PeerParams) -> Arc<Self> {
        let tracker_addr = params.tracker_addr.clone();
        let peer = Arc::new(Peer {
            params,
            tracker: Tracker::new(tracker_addr).await,
            peer_id: RefCell::new(None),
            peer_connections: RwLock::new(HashMap::new()),
            files: RwLock::new(HashMap::new()),
            files_peer_state: RwLock::new(HashMap::new()),
            interval_id: RefCell::new(None),
            update_handler: RefCell::new(None),
        });

        peer.init();

        peer
    }

    fn init(self: &Arc<Self>) {
        use crate::window;
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let self_weak = Arc::downgrade(self);
        self.tracker.set_handler(move |msg| {
            if let Some(self_arc) = self_weak.upgrade() {
                spawn_local(async move { self_arc.on_tracker_message(msg).await });
            }
        });

        let weak = Arc::downgrade(self);
        let update_handler: Box<dyn FnMut()> = Box::new(move || {
            if let Some(arc) = weak.upgrade() {
                spawn_local(async move { arc.update().await });
            }
        });
        let update_handler = Closure::wrap(update_handler);

        let prev = self.interval_id.replace(Some(
            window()
                .unwrap()
                .set_interval_with_callback_and_timeout_and_arguments_0(
                    update_handler.as_ref().unchecked_ref(),
                    self.params.peer_send_interval_ms as i32,
                )
                .unwrap(),
        ));
        assert!(prev.is_none());

        let prev = self.update_handler.replace(Some(update_handler));
        assert!(prev.is_none());
    }

    pub fn send(&self, message: PeerTrackerMessage) {
        self.tracker.send(message);
    }

    async fn on_tracker_message(self: &Arc<Self>, msg: TrackerPeerMessage) {
        match msg {
            TrackerPeerMessage::PeerIdAssigned { peer_id } => {
                let _: Option<_> = self.peer_id.replace(Some(peer_id));
            }
            TrackerPeerMessage::RequestOffer {
                peer_id,
                file_sha256,
            } => {
                let mut peer_connections = self.peer_connections.write().await;
                if !peer_connections.contains_key(&peer_id) {
                    let pc = Connection::new(self, peer_id).await;
                    let prev = peer_connections.insert(peer_id, Arc::clone(&pc));
                    assert!(prev.is_none());
                    pc.send_offer().await;
                };

                let file_peer_ids = self.get_or_insert_empty_file_peer_state(file_sha256).await;
                let mut file_peer_ids = file_peer_ids.write().await;
                if file_peer_ids.contains_key(&peer_id) {
                    let prev = file_peer_ids.insert(peer_id, None);
                    assert!(prev.is_none());
                }
            }
            TrackerPeerMessage::PeerOffer { peer_id, offer } => {
                let mut peer_connections = self.peer_connections.write().await;
                let pc = if let Some(pc) = peer_connections.get(&peer_id) {
                    Arc::clone(pc)
                } else {
                    let pc = Connection::new(self, peer_id).await;
                    let prev = peer_connections.insert(peer_id, Arc::clone(&pc));
                    assert!(prev.is_none());
                    pc
                };
                pc.on_peer_offer(offer).await;
            }
            TrackerPeerMessage::PeerAnswer { peer_id, answer } => {
                let peer_connections = self.peer_connections.read().await;
                if let Some(pc) = peer_connections.get(&peer_id) {
                    pc.on_peer_answer(answer).await;
                } else {
                    log::error!("unexpected answer from peer {}", peer_id);
                };
            }
            TrackerPeerMessage::PeerIceCandidate { peer_id, candidate } => {
                let peer_connections = self.peer_connections.read().await;
                if let Some(pc) = peer_connections.get(&peer_id) {
                    pc.on_peer_icecandidate(candidate).await;
                } else {
                    log::error!("unexpected icecandidate from peer {}", peer_id);
                };
            }
            TrackerPeerMessage::PeerAllIceCandidatesSent { peer_id } => {
                let peer_connections = self.peer_connections.read().await;
                if let Some(pc) = peer_connections.get(&peer_id) {
                    pc.on_peer_all_icecandidates_sent().await;
                } else {
                    log::error!("unexpected all_icecandidates_sent from peer {}", peer_id);
                };
            }
        }
    }

    pub async fn on_peer_message(
        self: &Arc<Self>,
        other_peer_id: PeerId,
        message: PeerPeerMessage,
    ) {
        use crate::{body, window, ElementExt, FileSetChunkError};
        use js_sys::{Array, Function, Reflect, Uint8Array};
        use wasm_bindgen::{JsCast, JsValue};
        use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Window};

        match message {
            PeerPeerMessage::FileMetaData { sha256, name, len } => {
                let mut files = self.files.write().await;
                if let Some(file) = files.get(&sha256) {
                    let file = file.read().await;
                    if file.name() != &name || file.len() != len {
                        log::error!("different file metadata for the same sha256 found");
                    }
                } else {
                    let file = Arc::new(RwLock::new(File::new(name, len)));
                    let prev = files.insert(sha256, file);
                    assert!(prev.is_none());
                }
            }
            PeerPeerMessage::FileState { sha256, chunks } => {
                let file_peer_ids = self.get_or_insert_empty_file_peer_state(sha256).await;
                let _: Option<_> = file_peer_ids
                    .write()
                    .await
                    .insert(other_peer_id, Some(chunks));
            }
            PeerPeerMessage::FileChunk {
                sha256,
                chunk_idx,
                bytes,
            } => {
                let file = self.files.write().await.get(&sha256).map(Arc::clone);
                if let Some(file) = file {
                    let mut file = file.write().await;
                    let file_ready_before = file.is_ready();
                    match file.set_chunk(chunk_idx, &bytes) {
                        Ok(()) => {
                            // TODO: files stats
                        }
                        Err(FileSetChunkError::ChunkIsAlreadySet { .. }) => {}
                        Err(err) => {
                            log::error!("{}", err)
                        }
                    }
                    if !file_ready_before && file.is_ready() {
                        log::debug!("{} is fully downloaded...", file.name());

                        log::debug!("  making Uint8Array");
                        let array = Uint8Array::from(file.data());

                        //let mut blob_props = BlobPropertyBag::new();
                        //let _: &mut _ = blob_props.type_("text/plain");

                        let blob_args: Array = [array].iter().collect();
                        log::debug!("  marking Blob");
                        let blob =
                            Blob::new_with_u8_array_sequence(&blob_args /*, &blob_props*/).unwrap();

                        let window: Window = window().unwrap();
                        let url = Reflect::get(&window, &JsValue::from_str("URL")).unwrap();

                        // when using web_sys API
                        // Url::create_object_url_with_blob and HtmlAnchorElement::set_href
                        // performance and stability is drastically degraded,
                        // due to the transfer of blob url to wasm and back to js.
                        let create_object_url_fn: Function =
                            Reflect::get(&url, &JsValue::from_str("createObjectURL"))
                                .unwrap()
                                .dyn_into()
                                .unwrap();

                        let create_object_url_fn_args: Array = [blob].iter().collect();
                        log::debug!("  marking url");
                        let url =
                            Reflect::apply(&create_object_url_fn, &url, &create_object_url_fn_args)
                                .unwrap();

                        log::debug!("  add link");

                        log::debug!("  file is ready for downloading");

                        let link: HtmlAnchorElement = body().unwrap().add_child("a").unwrap();
                        link.add_text(&format!("Download {}", file.name())).unwrap();
                        let _: bool =
                            Reflect::set(&link, &JsValue::from_str("href"), &url).unwrap();
                        link.set_target("_blank");
                        link.set_download(file.name());
                    }
                } else {
                    log::warn!("file metadata is not yet received");
                }
            }
        }
    }

    pub async fn share_file(self: &Arc<Self>, name: String, bytes: Box<[u8]>) -> Option<Sha256> {
        let (sha256, new_file) = File::from(name, bytes);

        let mut files = self.files.write().await;
        if let Some(file) = files.get(&sha256) {
            let file = file.read().await;
            if file.name() != new_file.name() || file.len() != new_file.len() {
                log::error!("different file metadata for the same sha256 found");
            }
            None
        } else {
            let file = Arc::new(RwLock::new(new_file));
            let prev = files.insert(sha256, file);
            assert!(prev.is_none());
            self.send(PeerTrackerMessage::RequestOffers {
                file_sha256: sha256.clone(),
            });
            Some(sha256)
        }
    }

    pub async fn load_file(self: &Arc<Self>, sha256: Sha256) {
        if !self.files.read().await.contains_key(&sha256) {
            self.send(PeerTrackerMessage::RequestOffers {
                file_sha256: sha256,
            });
        }
    }

    pub async fn update(self: Arc<Self>) {
        use crate::CHUNK_SIZE;
        use rand::rngs::OsRng;
        use rand::Rng;

        for (sha256, file) in self.files.read().await.iter() {
            let file = file.read().await;
            log::debug!(
                "{}: {}% ({}/{})",
                file.name(),
                file.chunks().count_ones() * 100 / file.chunks().len(),
                file.chunks().count_ones(),
                file.chunks().len()
            );
            for (_, pc) in self.peer_connections.read().await.iter() {
                if !pc.is_ready() {
                    continue;
                }
                let _: Option<()> = pc
                    .send(
                        PeerPeerMessage::FileState {
                            sha256: sha256.clone(),
                            chunks: file.chunks().to_bitvec().into_boxed_bitslice(),
                        },
                        Some(self.params.max_channel_buffer_bytes),
                    )
                    .ok();
                let _: Option<()> = pc
                    .send(
                        PeerPeerMessage::FileMetaData {
                            sha256: sha256.clone(),
                            name: file.name().to_owned(),
                            len: file.len(),
                        },
                        Some(self.params.max_channel_buffer_bytes),
                    )
                    .ok();
            }
        }

        let mut tasks = Vec::new();
        let files_peer_state = self.files_peer_state.read().await;
        for (sha256, file) in self.files.read().await.iter() {
            let sha256_arc = Arc::new(sha256.clone());
            let chunks_len = file.read().await.chunks_len();
            let files_peer_state = files_peer_state.get(sha256);
            if let Some(files_peer_state) = files_peer_state {
                for (&other_peer_id, other_peer_mask) in files_peer_state.read().await.iter() {
                    if let Some(other_peer_mask) = other_peer_mask {
                        for chunk_idx in 0..chunks_len {
                            let other_bit =
                                *other_peer_mask.get(chunk_idx).as_deref().unwrap_or(&false);
                            if file.read().await.has_chunk(chunk_idx).unwrap() && !other_bit {
                                tasks.push((
                                    Arc::clone(&sha256_arc),
                                    Arc::clone(file),
                                    other_peer_id,
                                    chunk_idx,
                                ));
                            }
                        }
                    } else {
                        for chunk_idx in 0..chunks_len {
                            tasks.push((
                                Arc::clone(&sha256_arc),
                                Arc::clone(file),
                                other_peer_id,
                                chunk_idx,
                            ));
                        }
                    }
                }
            }
        }

        let mut bytes_sent = 0;
        let bytes_limit = (self.params.upload_speed_limit_bps * self.params.peer_send_interval_ms
            / 1000) as usize;

        let mut logs = HashMap::new();
        while bytes_sent < bytes_limit && !tasks.is_empty() {
            let task_idx = OsRng.gen_range(0..tasks.len());
            let (sha256, file, peer_id, chunk_idx) = tasks.remove(task_idx);

            let pc = self.peer_connections.read().await.get(&peer_id).cloned();
            let pc = match pc {
                Some(pc) => pc,
                None => continue,
            };
            if !pc.is_ready() {
                continue;
            }
            let file = file.read().await;
            let _: Option<()> = pc
                .send(
                    PeerPeerMessage::FileChunk {
                        sha256: sha256.as_ref().to_owned(),
                        chunk_idx,
                        bytes: file
                            .get_chunk(chunk_idx)
                            .unwrap()
                            .unwrap()
                            .to_owned()
                            .into_boxed_slice(),
                    },
                    Some(self.params.max_channel_buffer_bytes),
                )
                .ok();
            let logs: &mut Vec<_> = logs.entry((file.name().to_owned(), peer_id)).or_default();
            logs.push(chunk_idx);
            // TODO: files_stats
            bytes_sent += CHUNK_SIZE;
        }
        for ((file, peer_id), chunks) in logs {
            log::debug!(
                "{}: send to peer `{}` {} chunks",
                file,
                peer_id,
                chunks.len()
            );
        }
    }

    async fn get_or_insert_empty_file_peer_state(
        &self,
        file_sha256: Sha256,
    ) -> Arc<RwLock<HashMap<PeerId, Option<BitBox>>>> {
        let mut files_peers = self.files_peer_state.write().await;
        let file_peers = files_peers.get(&file_sha256);
        match file_peers {
            Some(file_peers) => Arc::clone(file_peers),
            None => {
                let file_peers = Arc::new(RwLock::new(HashMap::new()));
                let prev = files_peers.insert(file_sha256, Arc::clone(&file_peers));
                assert!(prev.is_none());
                file_peers
            }
        }
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        use crate::window;

        if let Some(interval_id) = self.interval_id.borrow().clone() {
            if let Ok(window) = window() {
                window.clear_interval_with_handle(interval_id);
            }
        }
    }
}
