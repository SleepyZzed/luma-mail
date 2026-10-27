//! Real Linux/WebKit editing checks with synthetic data and no online account.
use super::*;
use std::time::Duration;
async fn js(e: &Editor, code: &str) -> String {
    let (tx, rx) = tokio::sync::oneshot::channel();
    e.rich.eval(code.into(), move |result| {
        let _ = tx.send(result);
    });
    rx.await.unwrap().unwrap()
}
async fn snapshot(e: &Editor) -> String {
    let (tx, rx) = tokio::sync::oneshot::channel();
    e.rich.snapshot(move |result| {
        let _ = tx.send(result);
    });
    rx.await.unwrap().unwrap()
}
async fn wait_ready(e: &Editor) {
    for _ in 0..200 {
        if e.rich.is_ready() {
            return;
        }
        glib::timeout_future(Duration::from_millis(50)).await;
    }
    panic!("Signature editor did not become ready");
}
fn button(widget: &gtk::Widget, title: &str) -> Option<gtk::Button> {
    if widget.tooltip_text().as_deref() == Some(title) {
        return widget.clone().downcast().ok();
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(found) = button(&widget, title) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}
fn response(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    if let Some(b) = widget.downcast_ref::<gtk::Button>()
        && b.label().as_deref() == Some(label)
    {
        return Some(b.clone());
    }
    let mut child = widget.first_child();
    while let Some(w) = child {
        if let Some(b) = response(&w, label) {
            return Some(b);
        }
        child = w.next_sibling();
    }
    None
}
pub(super) fn start(s: &Rc<State>) {
    let state = s.clone();
    glib::MainContext::default().spawn_local(async move {
        let account=state.settings.borrow().accounts[0].clone();
        let signature=Signature{identity:account.email.clone(),html:"<div>Alex Morgan</div><div>Product designer</div>".into(),can_update:true};
        let e=loaded(&state,account,signature,None).unwrap();
        wait_ready(&e).await;
        glib::timeout_future(Duration::from_millis(400)).await;
        for title in ["Bold","Italic","Underline","Bullet list","Numbered list","Insert hyperlink","Insert picture from a public HTTPS image URL"] {
            let b=button(e.window.upcast_ref(),title).expect(title);
            assert!(b.is_mapped()&&b.width()>0,"{title} must be visible");
            let flow=b.parent().unwrap().parent().unwrap();
            let bounds=b.compute_bounds(&flow).unwrap();
            assert!(bounds.x()>=0.&&bounds.x()+bounds.width()<=flow.width() as f32+1.,"{title} is clipped");
        }
        assert_eq!(js(&e,"(()=>{const r=document.createRange();r.selectNodeContents(document.getElementById('luma-signature').firstChild);getSelection().removeAllRanges();getSelection().addRange(r);return true;})()").await,"true");
        button(e.window.upcast_ref(),"Bold").unwrap().emit_clicked();
        js(&e,"true").await;
        assert!(snapshot(&e).await.contains("font-weight")||snapshot(&e).await.contains("<b>"));
        apply(&e,"fontName","Arial");
        apply(&e,"foreColor","#2456ab");
        apply(&e,"createLink","https://example.com/contact");
        js(&e,"(()=>{const r=document.createRange();r.selectNodeContents(document.getElementById('luma-signature').lastChild);getSelection().removeAllRanges();getSelection().addRange(r);return true;})()").await;
        button(e.window.upcast_ref(),"Bullet list").unwrap().emit_clicked();
        js(&e,"true").await;
        let html=snapshot(&e).await;
        assert!(html.contains("<ul>")&&html.contains("https://example.com/contact")&&html.contains("Arial"),"Rich formatting lost: {html}");
        js(&e,"(()=>{const r=document.createRange();r.selectNodeContents(document.getElementById('luma-signature'));r.collapse(false);getSelection().removeAllRanges();getSelection().addRange(r);return true;})()").await;
        apply(&e,"fontSizePt","18");
        js(&e,"document.execCommand('insertText',false,'Signature size check');true").await;
        apply(&e,"insertTable","2x2");
        let picture=serde_json::json!({"url":"https://example.com/logo.png","data":luma_core::html_mail::image_data(include_bytes!("../../../data/luma-wordmark.png")).unwrap(),"width":100,"alt":"Synthetic logo"});
        apply(&e,"insertImage",&picture.to_string());
        js(&e,"true").await;
        for _ in 0..30 {if js(&e,"Boolean(document.querySelector('#luma-signature img')?.naturalWidth)").await=="true"{break;}glib::timeout_future(Duration::from_millis(50)).await;}
        assert_eq!(js(&e,"Boolean(document.querySelector('#luma-signature img')?.naturalWidth)").await,"true","Hosted picture preview must decode and display");
        assert_eq!(js(&e,"document.getElementById('luma-signature').dispatchEvent(new InputEvent('beforeinput',{data:'x',inputType:'insertText',cancelable:true,bubbles:true}))").await,"true","Preview image data must not consume the signature text budget");
        assert!(e.window.height()<=800,"Signature window should fit an ordinary desktop: {}",e.window.height());
        let html=snapshot(&e).await;
        assert!(html.contains("18pt")&&html.contains("Signature size check")&&html.contains("<table")&&html.contains("https://example.com/logo.png"),"Signature content lost: {html}");
        assert!(!html.contains("data:image")&&!html.contains("data-luma-source"));
        assert_eq!(js(&e,"JSON.stringify(window.lumaSignature.images())").await,"[\"https://example.com/logo.png\"]");
        let combined=luma_core::signatures::append("<div data-luma-editor=\"1\">New message</div>",&html).unwrap();
        let parsed=luma_core::rich_draft::parse(&combined).unwrap();
        assert_eq!(parsed.text,"New message");assert!(parsed.signature.contains("logo.png"));
        if let Some(path)=std::env::var_os("LUMA_SIGNATURE_SCREENSHOT") {
            glib::timeout_future(Duration::from_millis(300)).await;
            let paintable=gtk::WidgetPaintable::new(Some(&e.window));let snap=gtk::Snapshot::new();
            paintable.snapshot(&snap,e.window.width() as f64,e.window.height() as f64);
            let node=snap.to_node().expect("signature snapshot");
            e.window.renderer().unwrap().render_texture(&node,None).save_to_png(path).unwrap();
        }
        e.window.close();
        glib::timeout_future(Duration::from_millis(120)).await;
        let d=e.window.visible_dialog().unwrap().downcast::<adw::AlertDialog>().unwrap();
        response(d.upcast_ref(),"Keep editing").expect("Actual cancel button").emit_clicked();
        glib::timeout_future(Duration::from_millis(350)).await;
        assert!(!e.allow_close.get()&&!e.checking_close.get());
        e.window.close();
        for _ in 0..50 {if e.window.visible_dialog().is_some(){break;}glib::timeout_future(Duration::from_millis(50)).await;}
        let d=e.window.visible_dialog().unwrap().downcast::<adw::AlertDialog>().unwrap();
        response(d.upcast_ref(),"Close without saving").expect("Actual discard button").emit_clicked();
        let weak=Rc::downgrade(&e);drop(e);
        glib::timeout_future(Duration::from_millis(600)).await;
        assert!(weak.upgrade().is_none(),"Closed signature editor must release its renderer and owner");
        println!("PASS actual Linux signature toolbar, font/colour/link/bullets/table/picture, bounded hosted-image snapshot, separate draft signature, close confirmation and renderer release; no account or upload");
        state.app.quit();
    });
}
