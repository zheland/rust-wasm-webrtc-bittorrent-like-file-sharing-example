use std::sync::Arc;
use std::time::Duration;

use peer::LocalPeer;
use thiserror::Error;

use crate::{IntervalHandler, NewIntervalHandlerError, Time};

#[derive(Clone, Copy, Debug)]
pub struct SenderParams {
    pub data_send_interval: Duration,
    pub state_resend_interval: Duration,
    pub piece_resend_interval: Duration,
    pub num_pieces_to_be_sent: usize,
    pub max_buffer_bytes: Option<u64>,
}

#[derive(Debug)]
pub struct Sender {
    _handler: IntervalHandler,
}

impl Sender {
    pub fn new<F>(
        peer: Arc<LocalPeer<Time>>,
        params: SenderParams,
        update_callback: F,
    ) -> Result<Self, NewPeerSenderError>
    where
        F: 'static + Fn(),
    {
        use crate::{monotonic_time, JsRandom};
        use rand_chacha::ChaCha8Rng;
        use wasm_bindgen_futures::spawn_local;

        let update_callback = Arc::new(update_callback);
        let callback = move || {
            let update_callback = Arc::clone(&update_callback);
            let peer = Arc::clone(&peer);
            spawn_local(async move {
                let time = monotonic_time().unwrap();
                let rng = ChaCha8Rng::new();

                peer.send_state_to_remote_peers(
                    time.saturating_sub(params.state_resend_interval),
                    time,
                )
                .await;

                peer.send_recently_received_to_remote_peers().await;

                peer.resend_pieces_before(time.saturating_sub(params.piece_resend_interval))
                    .await;

                peer.send_pieces_to_remote_peers(
                    params.num_pieces_to_be_sent,
                    params.max_buffer_bytes,
                    time,
                    rng,
                )
                .await;

                update_callback();
            });
        };
        let handler = IntervalHandler::new(callback, params.data_send_interval)?;

        Ok(Self { _handler: handler })
    }
}

#[derive(Error, Debug)]
pub enum NewPeerSenderError {
    #[error(transparent)]
    NewIntervalHandlerError(#[from] NewIntervalHandlerError),
}
