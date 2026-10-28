//! Authored inline pictures: bounded RAM bytes, CID MIME parts and an explicit
//! editor acknowledgement before a queued Send may continue.
use crate::{
    app::App,
    compose::{Init, Session},
    compose_commands,
    protocol::Command,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use luma_core::{Error, Result, composer::RetainedAttachment};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
fn preview(file: &RetainedAttachment) -> Result<Value> {
    let image = luma_core::preview::thumbnail(&file.data, 256, 192)?;
    Ok(
        json!({"source":format!("cid:{}",file.content_id.as_deref().unwrap_or("").trim_matches(['<','>'])),"width":image.width,"height":image.height,"rgba":STANDARD.encode(image.rgba)}),
    )
}
pub async fn restore(init: &mut Init) -> Result<()> {
    let files: Vec<_> = init
        .form
        .retained
        .iter()
        .filter(|f| {
            f.content_id
                .as_deref()
                .is_some_and(|id| id.trim_start_matches('<').starts_with("luma-inline-"))
        })
        .take(8)
        .cloned()
        .collect();
    if files.is_empty() {
        return Ok(());
    }
    let pictures = tokio::task::spawn_blocking(move || {
        files
            .iter()
            .filter_map(|f| preview(f).ok())
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| Error::Invalid("Could not restore inline picture previews".into()))?;
    init.document["pictures"] = json!(pictures);
    Ok(())
}
pub async fn prepare(app: &Arc<App>, c: &Command, s: &Arc<Session>) -> Result<()> {
    let token = c.text("token").to_owned();
    // Arm this before file IO/decoding. Cancellation or a stalled UI cannot
    // leave a queued Send waiting indefinitely for an editor acknowledgement.
    expire_input(app, s, token.clone());
    compose_commands::emit(app, c.id, s, false).await;
    let path = PathBuf::from(c.text("path"));
    let result=tokio::task::spawn_blocking(move||{
        use std::io::Read;
        let file=std::fs::File::open(&path)?;if !file.metadata()?.is_file(){return Err(Error::Invalid("Choose an image file".into()));}
        let mut bytes=Vec::new();file.take(luma_core::preview::AUTO_IMAGE_BYTES+1).read_to_end(&mut bytes)?;
        if bytes.len() as u64>luma_core::preview::AUTO_IMAGE_BYTES{return Err(Error::Invalid("Inline pictures are limited to 8 MiB. Attach the file or use a Drive link instead.".into()));}
        let mut file=luma_core::clipboard_image::attachment(bytes,1)?;
        file.name=path.file_name().unwrap_or_default().to_string_lossy().chars().filter(|c|!c.is_control()).take(160).collect();
        file.content_id=Some(format!("<luma-inline-{}@luma.invalid>",luma_core::composer::new_id()));
        let picture=preview(&file)?;Ok((file,picture))
    }).await.map_err(|_|Error::Invalid("Could not decode the selected picture".into())).and_then(|r|r);
    let (file, picture) = match result {
        Ok(value) => value,
        Err(e) => {
            s.cancel_input(&token, &e.to_string());
            compose_commands::emit(app, c.id, s, false).await;
            app.events
                .emit(
                    c.id,
                    "inlineImageComplete",
                    json!({"session":s.id,"token":token,"ok":false}),
                )
                .await;
            compose_commands::resume_close(app, c.id, s).await?;
            return Err(e);
        }
    };
    let accepted = {
        let mut d = s.data.lock().unwrap();
        if !d.input_busy || d.input_token != token {
            return Ok(());
        }
        let total = d.form.retained.iter().map(|f| f.data.len()).sum::<usize>() + file.data.len();
        let count = d
            .form
            .retained
            .iter()
            .filter(|f| {
                f.content_id
                    .as_deref()
                    .is_some_and(|id| id.contains("luma-inline-"))
            })
            .count();
        if count >= 8 || total > luma_core::composer::MAX_ATTACHMENT_BYTES as usize {
            false
        } else {
            d.pending_inline = Some(file);
            d.status = "Inserting inline picture".into();
            true
        }
    };
    if !accepted {
        s.cancel_input(
            &token,
            "Inline picture memory limit reached; queued Send was cancelled",
        );
        compose_commands::emit(app, c.id, s, false).await;
        app.events
            .emit(
                c.id,
                "inlineImageComplete",
                json!({"session":s.id,"token":token,"ok":false}),
            )
            .await;
        compose_commands::resume_close(app, c.id, s).await?;
        return Err(Error::ResponseLimit);
    }
    app.events
        .emit(
            c.id,
            "inlineImageReady",
            json!({"session":s.id,"token":token,"picture":picture}),
        )
        .await;
    Ok(())
}
fn expire_input(app: &Arc<App>, s: &Arc<Session>, token: String) {
    let app = Arc::downgrade(app);
    let s = Arc::downgrade(s);
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        if let (Some(app), Some(s)) = (app.upgrade(), s.upgrade())
            && s.cancel_input(
                &token,
                "Inline image insertion timed out; queued Send was cancelled",
            )
        {
            compose_commands::emit(&app, 0, &s, false).await;
            app.events
                .emit(
                    0,
                    "inlineImageComplete",
                    json!({"session":s.id,"token":token,"ok":false}),
                )
                .await;
            compose_commands::resume_close(&app, 0, &s).await.ok();
        }
    });
}
pub async fn acknowledge(app: &Arc<App>, c: &Command, s: &Arc<Session>) -> Result<()> {
    let result = acknowledge_form(c, s);
    if let Err(error) = &result {
        s.cancel_input(c.text("token"), &error.to_string());
    }
    compose_commands::emit(app, c.id, s, false).await;
    app.events
        .emit(
            c.id,
            "inlineImageComplete",
            json!({"session":s.id,"token":c.text("token"),"ok":result.is_ok()}),
        )
        .await;
    compose_commands::resume_close(app, c.id, s).await?;
    result
}
fn acknowledge_form(c: &Command, s: &Session) -> Result<()> {
    let token = c.text("token");
    if c.action == "composeInsertImageCancel" {
        s.cancel_input(token, "Inline image cancelled; queued Send was cancelled");
    } else {
        let file = {
            let mut d = s.data.lock().unwrap();
            if !d.input_busy || d.input_token != token {
                return Err(Error::Invalid(
                    "This inline image transfer has ended".into(),
                ));
            }
            d.pending_inline
                .take()
                .ok_or_else(|| Error::Invalid("The inline image is not ready".into()))?
        };
        let cid = file.content_id.as_deref().unwrap().trim_matches(['<', '>']);
        if !c.data["form"]["html"].as_bool().unwrap_or(false)
            || !c.data["form"]["text"]
                .as_str()
                .unwrap_or("")
                .contains(&format!("cid:{cid}"))
        {
            s.cancel_input(
                token,
                "Inline image insertion failed; queued Send was cancelled",
            );
            return Err(Error::Invalid(
                "The picture was not inserted in the message body".into(),
            ));
        }
        if let Err(e) = s.update(&c.data["form"], c.id) {
            s.cancel_input(
                token,
                "Inline image insertion failed; queued Send was cancelled",
            );
            return Err(e);
        }
        if let Err(e) = s.accept_input(token, file).and_then(|accepted| {
            if accepted {
                Ok(())
            } else {
                Err(Error::Invalid(
                    "This inline image transfer has ended".into(),
                ))
            }
        }) {
            s.cancel_input(
                token,
                "Inline image limit reached; queued Send was cancelled",
            );
            return Err(e);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::{Intent, delivery};
    fn editor() -> Arc<Session> {
        let mut form = luma_core::composer::Compose::new("sender@example.invalid".into());
        form.to = "recipient@example.invalid".into();
        Session::new(Init {
            form,
            primary: "sender@example.invalid".into(),
            mode: "new".into(),
            key: None,
            source_message: None,
            draft: None,
            quote: String::new(),
            signature: String::new(),
            original_html: None,
            source_mode: false,
            document: json!({"kind":"plain","text":""}),
        })
    }
    fn stage(s: &Session, token: &str) {
        s.begin_input(token).unwrap();
        s.data.lock().unwrap().pending_inline = Some(RetainedAttachment {
            name: "inline.png".into(),
            mime_type: "image/png".into(),
            data: vec![1, 2, 3].into(),
            content_id: Some("<luma-inline-test@luma.invalid>".into()),
        });
    }
    #[test]
    fn queued_send_requires_editor_acknowledgement_and_cancel_drops_image_bytes() {
        let s = editor();
        stage(&s, "insert1");
        s.queue(1, Intent::Send).unwrap();
        assert!(s.data.lock().unwrap().input_busy);
        let cmd = Command {
            id: 2,
            action: "composeInsertImageAck".into(),
            data: json!({"token":"insert1","form":{"html":true,"text":"<p>Before</p><img src=\"cid:luma-inline-test@luma.invalid\"><p>After</p>"}}),
        };
        acknowledge_form(&cmd, &s).unwrap();
        let d = s.data.lock().unwrap();
        assert!(!d.input_busy);
        assert_eq!(d.form.retained.len(), 1);
        assert!(d.pending.is_some());
        assert!(delivery(&d).unwrap().text.contains("cid:luma-inline-test"));
        drop(d);
        let s = editor();
        stage(&s, "insert2");
        s.queue(3, Intent::Send).unwrap();
        acknowledge_form(
            &Command {
                id: 4,
                action: "composeInsertImageCancel".into(),
                data: json!({"token":"insert2"}),
            },
            &s,
        )
        .unwrap();
        let d = s.data.lock().unwrap();
        assert!(
            !d.input_busy
                && d.pending.is_none()
                && d.pending_inline.is_none()
                && d.form.retained.is_empty()
        );
    }
    #[test]
    fn stale_or_missing_body_acknowledgements_never_accept_an_orphan_picture() {
        let s = editor();
        stage(&s, "current");
        assert!(
            acknowledge_form(
                &Command {
                    id: 1,
                    action: "composeInsertImageAck".into(),
                    data: json!({"token":"previous","form":{"html":true,"text":""}})
                },
                &s
            )
            .is_err()
        );
        assert!(s.data.lock().unwrap().input_busy);
        assert!(
            acknowledge_form(
                &Command {
                    id: 2,
                    action: "composeInsertImageAck".into(),
                    data: json!({"token":"current","form":{"html":true,"text":"No picture"}})
                },
                &s
            )
            .is_err()
        );
        assert!(s.data.lock().unwrap().form.retained.is_empty());
        s.update(
            &json!({"html":true,"text":"<img src=\"cid:luma-inline-missing@luma.invalid\">"}),
            3,
        )
        .unwrap();
        assert!(delivery(&s.data.lock().unwrap()).is_err());
    }
}
