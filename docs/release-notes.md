Luma Mail 0.8 early access — online rich signatures and explicit drafting

- Restore individually wrapped rich formatting controls, including bullets/numbering, in compact and inline composers.
- Settings → Storage → Drafts offers Ask on close (default), Save on close only with a valid recipient and authored body, or Manual Save only. Typing/attaching do not start background saves.
- Settings → Accounts → Rich signature opens the Linux visual editor: installed fonts, sizing, colours, highlighting, bold/italic/underline/strikeout, lists, indentation, alignment, line spacing, links, tables, pictures and undo/redo.
- Save the primary address's signature online in Gmail, with optional basic-settings consent and a fresh server-change check. This updates Gmail web's signature too. Alias signatures can be read; Gmail's named-signature library is not exposed by this integration.
- Local signature pictures require explicit confirmation for a public, unlisted, read-only optimized Drive copy, with progress and Cancel. Public HTTPS picture URLs also work. Preview/editing stays bounded in RAM; no local image/signature archive.
- Signatures stay separate from authored text and quoted history, preserve draft versions and do not multiply on repeated saves. Provider-neutral HTML/storage interfaces leave room for future providers; Gmail is the implemented provider today.

See docs/signatures.md for usage, image privacy and API limitations. Quit the previous version through its tray menu before launching this build. Google verification and target Linux/two-account/long-soak acceptance remain outstanding. The RAM goals in docs/performance.md are not claimed achieved. This is early access.
