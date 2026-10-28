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
