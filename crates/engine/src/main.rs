//! Private child-process service. The UI owns its anonymous pipes; no listening
//! socket, mailbox database, web server or plaintext token hand-off is introduced.
mod accounts;
mod app;
mod attachment_preview;
mod attachment_source;
mod clipboard;
mod compose;
mod compose_commands;
mod draft_conflict;
mod labels;
mod mail;
mod mail_commands;
mod photos;
mod protocol;
mod reader;
mod resident;
mod search_commands;
mod sync;
use luma_core::registration;

use luma_core::Result;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("mail runtime");
    if let Err(error) = runtime.block_on(run()) {
        // Never log a command, body, recipient, token, attachment path or raw HTTP.
        eprintln!("Luma engine stopped: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let (events, mut writer) = protocol::Events::start();
    let app = app::App::open(events.clone()).await?;
    app.boot().await;
    let mut input = BufReader::new(tokio::io::stdin());
    loop {
        let mut frame = Vec::new();
        // Bound a frame before parsing it, including clipboard image input.
        let n = (&mut input)
            .take(protocol::MAX_COMMAND as u64 + 1)
            .read_until(b'\n', &mut frame)
            .await?;
        if n == 0 {
            break;
        }
        if n > protocol::MAX_COMMAND || frame.last() != Some(&b'\n') {
            return Err(luma_core::Error::ResponseLimit);
        }
        match serde_json::from_slice::<protocol::Command>(&frame) {
            Ok(command) => {
                if command.action == "quit" {
                    break;
                }
                app.clone().dispatch(command);
            }
            Err(_) => events.error(0, "Invalid UI command").await,
        }
    }
    app.shutdown().await;
    drop(app);
    drop(events);
    if tokio::time::timeout(std::time::Duration::from_secs(2), &mut writer)
        .await
        .is_err()
    {
        writer.abort();
    }
    Ok(())
}
