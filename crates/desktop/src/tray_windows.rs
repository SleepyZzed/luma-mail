//! Native Explorer tray. A blocking Win32 message loop sleeps when idle;
//! the GTK side wakes only for shell events, with no polling timer.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicIsize, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Shell::*, WindowsAndMessaging::*},
};
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const CALLBACK: u32 = WM_APP + 9;
const OPEN: usize = 100;
const QUIT: usize = 101;
enum Event {
    Show,
    Quit,
    Available(bool),
}
struct Tray {
    events: tokio::sync::mpsc::Sender<Event>,
    icon: HICON,
    taskbar: u32,
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn data(hwnd: HWND, icon: HICON) -> NOTIFYICONDATAW {
    let mut n: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    n.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = hwnd;
    n.uID = 1;
    n.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
    n.hIcon = icon;
    n.uCallbackMessage = CALLBACK;
    for (dst, src) in n.szTip.iter_mut().zip(wide("Luma Mail")) {
        *dst = src;
    }
    n
}
unsafe fn add(hwnd: HWND, t: &Tray) -> bool {
    unsafe {
        let mut n = data(hwnd, t.icon);
        let ok = Shell_NotifyIconW(NIM_ADD, &n) != 0;
        if ok {
            n.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &n);
        }
        ok
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Tray;
        if let Some(t) = ptr.as_ref() {
            if message == t.taskbar {
                let _ = t.events.try_send(Event::Available(add(hwnd, t)));
                return 0;
            }
            if message == WM_COMMAND {
                if w == OPEN {
                    let _ = t.events.try_send(Event::Show);
                } else if w == QUIT {
                    let _ = t.events.try_send(Event::Quit);
                }
                return 0;
            }
            if message == CALLBACK {
                match (l as u32) & 0xffff {
                    WM_LBUTTONUP | NIN_SELECT | NIN_KEYSELECT => {
                        let _ = t.events.try_send(Event::Show);
                    }
                    WM_CONTEXTMENU | WM_RBUTTONUP => {
                        let menu = CreatePopupMenu();
                        AppendMenuW(menu, MF_STRING, OPEN, wide("Open Luma Mail").as_ptr());
                        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                        AppendMenuW(menu, MF_STRING, QUIT, wide("Quit").as_ptr());
                        let mut p = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut p);
                        SetForegroundWindow(hwnd);
                        let id = TrackPopupMenu(
                            menu,
                            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                            p.x,
                            p.y,
                            0,
                            hwnd,
                            std::ptr::null(),
                        ) as usize;
                        DestroyMenu(menu);
                        PostMessageW(hwnd, WM_NULL, 0, 0);
                        if id == OPEN {
                            let _ = t.events.try_send(Event::Show);
                        } else if id == QUIT {
                            let _ = t.events.try_send(Event::Quit);
                        }
                    }
                    _ => (),
                }
                return 0;
            }
            if message == WM_CLOSE {
                DestroyWindow(hwnd);
                return 0;
            }
            if message == WM_DESTROY {
                Shell_NotifyIconW(NIM_DELETE, &data(hwnd, t.icon));
                PostQuitMessage(0);
                return 0;
            }
        }
        DefWindowProcW(hwnd, message, w, l)
    }
}
pub fn start(s: &Rc<State>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let handle = Arc::new(AtomicIsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let thread_handle = handle.clone();
    let thread_stop = stop.clone();
    std::thread::Builder::new()
        .name("luma-windows-tray".into())
        .spawn(move || unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let class = wide("LumaMailTrayHost");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: module,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            if RegisterClassW(&wc) == 0 {
                let _ = tx.try_send(Event::Available(false));
                return;
            }
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class.as_ptr(),
                wide("Luma Mail").as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                module,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                let _ = tx.try_send(Event::Available(false));
                return;
            }
            let mut pixels = include_bytes!("../../../data/tray-icon.argb").to_vec();
            for p in pixels.as_chunks_mut::<4>().0 {
                p.rotate_left(1);
                p.swap(0, 2);
            }
            let mask = [0u8; 128];
            let icon = CreateIcon(module, 32, 32, 1, 32, mask.as_ptr(), pixels.as_ptr());
            let mut tray = Tray {
                events: tx,
                icon,
                taskbar: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            };
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut tray as *mut Tray as isize);
            thread_handle.store(hwnd as isize, Ordering::Release);
            let available = add(hwnd, &tray);
            let _ = tray.events.try_send(Event::Available(available));
            if thread_stop.load(Ordering::Acquire) {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            thread_handle.store(0, Ordering::Release);
            DestroyIcon(icon);
            UnregisterClassW(class.as_ptr(), module);
        })
        .expect("Windows tray thread");
    #[cfg(debug_assertions)]
    if std::env::args().any(|a| a == "--windows-shell-diagnostics") {
        diagnostic(s, handle.clone());
    }
    s.app.connect_shutdown(move |_| {
        stop.store(true, Ordering::Release);
        let hwnd = handle.load(Ordering::Acquire);
        if hwnd != 0 {
            unsafe {
                PostMessageW(hwnd as HWND, WM_CLOSE, 0, 0);
            }
        }
    });
    let weak = Rc::downgrade(s);
    gtk::glib::MainContext::default().spawn_local(async move {
        while let Some(event) = rx.recv().await {
            let Some(s) = weak.upgrade() else { break };
            match event {
                Event::Show => show(&s, None),
                Event::Quit => quit(&s),
                Event::Available(available) => {
                    s.tray_available.set(available);
                    if !available && !s.window.is_visible() {
                        show(&s, None);
                    }
                }
            }
        }
    });
}

#[cfg(debug_assertions)]
fn diagnostic(s: &Rc<State>, handle: Arc<AtomicIsize>) {
    let state = s.clone();
    gtk::glib::MainContext::default().spawn_local(async move{
        for _ in 0..100{if state.tray_available.get(){break}gtk::glib::timeout_future(std::time::Duration::from_millis(50)).await;}
        assert!(state.tray_available.get(),"Windows shell must accept the tray icon");
        let hwnd=handle.load(Ordering::Acquire) as HWND;let mut id:NOTIFYICONIDENTIFIER=unsafe{std::mem::zeroed()};id.cbSize=std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32;id.hWnd=hwnd;id.uID=1;
        let mut rect=RECT::default();assert_eq!(unsafe{Shell_NotifyIconGetRect(&id,&mut rect)},0,"Explorer must expose the real tray icon rectangle");
        state.window.set_visible(false);unsafe{PostMessageW(hwnd,CALLBACK,0,NIN_SELECT as isize);}
        gtk::glib::timeout_future(std::time::Duration::from_millis(150)).await;assert!(state.window.is_visible());
        state.settings.borrow_mut().close_to_tray=true;state.window.close();gtk::glib::timeout_future(std::time::Duration::from_millis(150)).await;assert!(!state.window.is_visible());
        unsafe{PostMessageW(hwnd,WM_COMMAND,OPEN,0);}gtk::glib::timeout_future(std::time::Duration::from_millis(150)).await;assert!(state.window.is_visible());
        let n=gtk::gio::Notification::new("Luma Mail notification test");n.set_body(Some("Synthetic mail notification from the native Windows shell backend."));state.app.send_notification(Some("luma-shell-test"),&n);
        assert!(crate::presentation::window_chrome(false),"Windows always keeps its title bar");
        println!("PASS real Explorer tray registration, activation, close-to-tray and Open action; Windows title bar enabled; native GIO Windows notification dispatched (visual OS delivery is a desktop acceptance check)");
        state.app.quit();
    });
}
