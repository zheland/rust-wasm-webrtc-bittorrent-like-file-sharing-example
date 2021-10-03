use std::io;
use std::sync::Arc;

use async_std::net::TcpListener;
use thiserror::Error;

use crate::State;

#[derive(Debug)]
pub struct Tracker {
    listener: TcpListener,
    state: Arc<State>,
}

impl Tracker {
    pub async fn new<Address: AsRef<str>>(addr: Address) -> Result<Self, NewServerError> {
        let listener = TcpListener::bind(addr.as_ref()).await?;
        let state = Arc::new(State::new());

        log::info!("started on address: {}", addr.as_ref());

        Ok(Self { listener, state })
    }

    pub async fn run(self) {
        use crate::Socket;
        use async_std::task::{spawn, JoinHandle};

        while let Ok((stream, addr)) = self.listener.accept().await {
            let state = Arc::clone(&self.state);
            let _: JoinHandle<()> = spawn(async move {
                let socket = Socket::new(stream, addr, state).await;
                let socket = match socket {
                    Ok(socket) => socket,
                    Err(err) => {
                        log::error!("new socket {} error: {}", addr, err);
                        return;
                    }
                };
                match socket.run().await {
                    Ok(()) => {}
                    Err(err) => {
                        log::error!("socket {} run error: {}", addr, err);
                    }
                }
            });
        }
    }
}

#[derive(Error, Debug)]
pub enum NewServerError {
    #[error("TcpListener bind error: {0}")]
    TcpListenerBindError(#[from] io::Error),
}
