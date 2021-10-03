use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use crate::{IntervalHandler, LocalPeer, NewIntervalHandlerError};

#[derive(Clone, Copy, Debug)]
pub struct PeerSenderParams {
    pub data_send_interval: Duration,
    pub state_resend_interval: Duration,
    pub num_pieces_to_be_sent: usize,
    pub max_buffer_bytes: Option<u64>,
}

#[derive(Debug)]
pub struct PeerSender {
    handler: IntervalHandler,
}

impl PeerSender {
    pub fn new(peer: Arc<LocalPeer>, params: PeerSenderParams) -> Result<Self, NewPeerSenderError> {
        use crate::{IgnoreEmpty, OkOrLog};
        use async_std::sync::Weak;
        use wasm_bindgen_futures::spawn_local;

        let callback = move || {
            let peer = Arc::clone(&peer);
            spawn_local(async move {
                // TODO: Remove debugging
                for file in peer.files().read().await.values().filter_map(Weak::upgrade) {
                    let state = file.sharing_state().await;
                    let state = state.local_state();
                    log::debug!(
                        "{}: {}/{}",
                        file.metadata().name(),
                        state.num_available(),
                        state.len()
                    );
                }

                peer.send_state_to_remote_peers(params.state_resend_interval)
                    .await
                    .ok_or_log()
                    .ignore_empty();
                peer.sent_pieces_to_remote_peers(
                    params.num_pieces_to_be_sent,
                    params.max_buffer_bytes,
                )
                .await
                .ok_or_log()
                .ignore_empty();
            });
        };
        let handler = IntervalHandler::new(callback, params.data_send_interval)?;

        Ok(Self { handler })
    }
}

#[derive(Error, Debug)]
pub enum NewPeerSenderError {
    #[error(transparent)]
    NewIntervalHandlerError(#[from] NewIntervalHandlerError),
}
