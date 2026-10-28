Luma Mail 0.9 early access — Windows parity, richer composition and one Google consent flow

- Linux remains primary. Windows x64 gets a portable ZIP with the runtime DLL closure, star icon, native Explorer tray, Windows shell notifications, Credential Manager, visual online signatures and in-memory media previews. Windows keeps a title bar with minimise/maximise/close; Linux keeps its compositor-managed layout.
- Choose Gmail and optional Contacts, limited Drive and basic Gmail settings together when connecting an account. Granted permissions determine which features are enabled; skipping an optional permission no longer rejects Gmail sign-in. Connect skipped features later in Settings.
- Compose with text colours, highlighting/custom colours, paragraph alignment, line spacing/indent, superscript/subscript and monospace, alongside the restored bold/italic/underline, fonts, links and bullet/numbered lists. Formatting survives new Luma Gmail drafts and bounded undo/redo.
- Find/replace in the composer (Ctrl+F / Ctrl+H), including case and whole-word matching, Unicode offsets and one-step replacement undo. Search only runs while its bar is open.
- Source/visual switching preserves supported rich formatting and asks before flattening unsupported HTML.
- Release closed compact/inline composers and recipient popovers completely. Clear keyboard focus before teardown. Windows HTML uses resize/scroll/load events instead of permanent frame and 60 ms polling callbacks, and hides underneath Settings.
- Shared visual signatures now include bounded formatting history without storing image previews in every history entry; no local signature archive.

Extract the entire Windows ZIP and run luma-mail.exe. It needs the official Microsoft Edge WebView2 Evergreen Runtime (already on most current Windows systems); START-HERE.txt links the official installer. No Rust/MSYS2 installation is needed. The Windows binary is unsigned; release checksums are supplied.

Quit the previous build through its tray menu before launching this one. Google review, two-account target Linux acceptance, long-soak testing and the RAM targets remain release gates; this is not 1.0. The custom replacement for the GTK interface is queued for the next build, with existing functionality and the online-first storage rules preserved.


## v0.10.0-dev.2 — Fluid menus and composer fixes

- Center and pad action buttons; align typed text and placeholders consistently.
- Restore readable inbox right-click actions and themed editor context menus.
- Give composer/signature dialogs and menus spring entrance/exit with theme colours.
- Animate the same editor into its native pop-out window, including corner continuity.
- Close outgoing inline replies through draft policy; detached replies remain open
  and leave no placeholder in another conversation.
- Add keyboard context actions and release editing menus when closed.

Published a23a207 packages pass 296 Linux/299 Windows portable checks and both
platform/component/core gates. Storage/cache policy is unchanged. This is an
EA preview; real Linux/two-account, remaining authoring/Drive, RAM and Google/
security acceptance still precede 1.0. See docs/fluid-parity.md and roadmap.md.


## v0.10.0-dev.3 — UTF-8 caption follow-up

Includes the dev.2 button/menu/composer fixes and corrects three captions damaged
by Windows codepage text I/O. A source guard now rejects those encoding mistakes
in the QML, README and Markdown documentation. The mail engine, cache policy and
feature behavior are unchanged. Clean-source bbc341a packages pass all fifteen
portable groups (296 Linux / 299 Windows) and native/core/Clippy gates.


## v0.11.0-dev.1 — cross-platform hardening and inline pictures

Luma Mail 0.11 is an early-access build of the custom Qt Quick interface and the existing online-first Rust engine. Linux remains the priority. This is progress toward 1.0, with target-desktop, resource and Google-review gates still open.

### Fixes and additions

- Wayland startup prefers detected hardware Vulkan with graphics/swapchain support, with OpenGL/software preflight fallbacks. Explicit graphics overrides remain supported. The AppImage requests the installed desktop's native file-chooser portal; its Qt fallback follows Luma's palette.
- Send submits directly. Reader reconciliation cannot turn a submitted Send into a save-draft question. Close still follows the selected draft policy.
- Reply/forward composers provide Reply, Reply all, Forward single and Forward all. New messages keep those controls hidden. Whole-conversation forwarding keeps readable history outside authored input and preserves attachments within the existing limits: 100 messages, 128 KiB history, 32 forwarded files and 16 MiB attachment RAM.
- Insert picture in body uses CID images with proper related/alternative/mixed MIME. Inline pictures round-trip through Gmail drafts; originals remain in bounded RAM, while only small thumbnails enter the editor. Send waits for insertion acknowledgment. Ordinary attached pictures remain attachments and recipient clients may place them after quoted history. Authoring preserves default font styling and responsive image bounds; recipient clients still control wrapping/dark mode.
- Opening morphs scale cached text instead of changing font size every frame, and defer heavy body/HTML layout until projection handoff. Software rendering has an icon fallback.
- Normal launches have one per-profile owner. Preview/test windows register no tray icons. Repeated Windows activation no longer drops its final named-pipe acknowledgment.
- Native notifications provide Open and, for unambiguous 4–8 digit codes in existing preview metadata, Copy code. Privacy mode suppresses codes. Targets are bounded and expire; actual OS notification appearance/buttons remain a desktop acceptance check.
- Settings offers opt-in external-image autoload and a separate attachment-thumbnail preference. Received images/previews are released on close/collapse/conversation change; no image archive is added.

### Storage and validation

No local mailbox mirror, search index or draft archive is added. The existing original-body cache remains bounded (100 MiB default, including a zero-cache option), with the same TTL/LRU and cleanup rules. Explicit downloads remain user-selected exports. Framework installation size is separate from runtime mail storage.

Source: `d04f531b748ab707a5751f9391dba3f1cbc4654e`. Both portable packages pass the [exact-source build and test run](https://github.com/SleepyZzed/luma-mail/actions/runs/40718748514): 308 Windows and 305 Linux checks across fifteen groups, plus native component tests, core/engine tests and strict Clippy. Package/source provenance is verified; public asset hashes and the tag are checked separately.

A live Windows run using the two authorized Gmail accounts passes 29 checks: bounded inline/ordinary attachments, one online draft, reopen/send, recipient delivery, inline-image resolution, reply threading/unread state, whole-conversation forwarding, star/importance/read actions, archive/inbox/Trash/restore and sent-draft removal. Disposable fixtures were moved to Trash; the zero-cache profile wrote no mail files. This tests Gmail transport and MIME, not visual rendering on a phone.

The exact-source synthetic CI process-tree audits record Windows private working set of 66.7 MiB at baseline, 80.0 MiB after scrolling 1,000 rows and 142.3 MiB with HTML. Linux, using software Mesa and Chromium GPU disabled, records PSS of 227.8 MiB at baseline and 402.5 MiB with HTML. Settled sampled CPU is 0.000% of one core. The counters/platforms are not directly comparable; Linux RAM targets and sustained real-mail usage remain open. The separate local debug/Qt 6.12 profile has its own higher measurements, retained in the audit.

Real Linux GPU/Wayland, chooser, keyring, Noctalia, notification and two-account acceptance remain open because the maintainer's Linux desktop is not remotely reachable from this Windows workspace. The two-account synthetic Linux fixtures are not described as live tests. See [the 1.0 audit](https://github.com/SleepyZzed/luma-mail/blob/d04f531b748ab707a5751f9391dba3f1cbc4654e/docs/v1-acceptance-0.11.md) for the remaining work, including Drive interruption/sharing recovery and independent security/privacy acceptance.

Google registration is bundled: users sign in through Google, with no OAuth JSON import. Google approval is unconfirmed; the unverified-app warning and lifetime-user cap still apply.

### Downloads

Linux: make the AppImage executable and launch it normally; test this build without the previous Wayland/Vulkan terminal overrides first. Windows: extract the entire ZIP and launch `bin/luma-mail.exe`, keeping its DLLs/plugins together. Windows requires the installed WebView2 runtime and retains its native title bar. Verify downloads against `SHA256SUMS.txt`. Source, notices and installation-footprint reports are included below.
