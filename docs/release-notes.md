Luma Mail 0.7.1 early access — composer controls and explicit draft saving

- Restore discoverable Bold, Italic, Underline, Strikethrough, bullets, numbered lists, links and Undo/Redo. Individual toolbar actions wrap to fit compact and inline composers instead of being clipped inside one grouped layout.
- Settings → Storage → Drafts now offers Ask on close (default), Save on close, and Manual Save only.
- Typing and attaching files no longer start background Gmail draft saves. Ask offers Keep editing, Close without saving, or Save draft.
- Save on close requires a valid recipient in To/Cc/Bcc and a nonempty authored message body. Otherwise the composer closes without an automatic draft. Quoted history does not count as a body.
- Manual mode saves only through the composer’s Save button. Closing drops unsaved edits and keeps any previously saved Gmail draft unchanged. Sending continues to send normally.
- Pending clipboard/import/save callbacks respect the close policy. Repeated Close opens one confirmation; it cannot queue repeated saves.

Quit the previous version through its tray menu before launching this build. Google verification and target desktop/long-soak acceptance remain outstanding. The measured RAM goals in docs/performance.md are not claimed achieved. This is early access.
