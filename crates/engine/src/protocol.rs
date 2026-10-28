use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::{
    io::AsyncWriteExt,
    sync::{OwnedSemaphorePermit, Semaphore, mpsc},
};

pub const MAX_COMMAND: usize = 4 * 1024 * 1024;
const EVENT_BUDGET: usize = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct Command {
    #[serde(default)]
    pub id: u64,
    pub action: String,
    #[serde(default)]
    pub data: Value,
}
impl Command {
    pub fn text(&self, name: &str) -> &str {
        self.data[name].as_str().unwrap_or("")
    }
    pub fn flag(&self, name: &str) -> bool {
        self.data[name].as_bool().unwrap_or(false)
    }
}
struct Frame {
    bytes: Vec<u8>,
    _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub struct Events {
    sender: mpsc::Sender<Frame>,
    budget: Arc<Semaphore>,
}
impl Events {
    pub fn start() -> (Self, tokio::task::JoinHandle<()>) {
        let (sender, mut receiver) = mpsc::channel::<Frame>(16);
        let events = Self {
            sender,
            budget: Arc::new(Semaphore::new(EVENT_BUDGET)),
        };
        let writer = tokio::spawn(async move {
            let mut output = tokio::io::stdout();
            while let Some(frame) = receiver.recv().await {
                if output.write_all(&frame.bytes).await.is_err()
                    || output.write_all(b"\n").await.is_err()
                    || output.flush().await.is_err()
                {
                    break;
                }
            }
        });
        (events, writer)
    }
    pub async fn emit(&self, id: u64, kind: &str, data: Value) {
        let Ok(mut bytes) = serde_json::to_vec(&json!({"id":id,"event":kind,"data":data})) else {
            return;
        };
        if bytes.len() >= EVENT_BUDGET {
            // Report the limit instead of leaving the UI in an endless loading
            // state after silently dropping a large body or metadata response.
            bytes=serde_json::to_vec(&json!({"id":id,"event":"error","data":{"message":"This content exceeds the in-memory display limit. Open it in Gmail or save the attachment explicitly."}})).unwrap();
        }
        let Ok(permit) = self
            .budget
            .clone()
            .acquire_many_owned(bytes.len().max(1) as u32)
            .await
        else {
            return;
        };
        let _ = self
            .sender
            .send(Frame {
                bytes,
                _permit: permit,
            })
            .await;
    }
    pub async fn error(&self, id: u64, text: &str) {
        self.emit(id, "error", json!({"message":text})).await;
    }
}
