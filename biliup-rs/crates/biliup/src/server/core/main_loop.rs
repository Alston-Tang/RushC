use crate::downloader::extractor::Site;
use crate::server::core::live_streamers::{Videos};
use crate::server::core::util::{Cycle};
use crate::server::core::StreamStatus;
use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::{channel, Receiver};
use tokio::task::JoinHandle;

/// This struct is used by client actors to send messages to the main loop. The
/// message type is `ToServer`.
#[derive(Clone, Debug)]
pub struct ServerHandle {
    chan: Sender<ToMain>,
}
impl ServerHandle {
    pub async fn send(&mut self, msg: ToMain) {
        if self.chan.send(msg).await.is_err() {
            panic!("Main loop has shut down.");
        }
    }
}

/// The message type used when a client actor sends messages to the main loop.
pub enum ToMain {
    NewRecording(Site, Cycle<StreamStatus>),
    FileClosed(Videos),
    // FatalError(io::Error),
}

pub fn spawn_main_loop() -> (ServerHandle, JoinHandle<()>) {
    let (send, recv) = channel(64);

    let handle = ServerHandle { chan: send };

    let join = tokio::spawn(async move {
        let res = main_loop(recv).await;
        match res {
            Ok(()) => {}
            Err(err) => {
                eprintln!("Oops {}.", err);
            }
        }
    });

    (handle, join)
}

async fn main_loop(mut recv: Receiver<ToMain>) -> Result<()> {
    while let Some(msg) = recv.recv().await {
        match msg {
            ToMain::NewRecording(_site, _task) => {}
            ToMain::FileClosed(_) => {}
        }
    }

    Ok(())
}
