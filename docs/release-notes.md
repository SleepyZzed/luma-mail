Luma Mail 0.6.0 early access — inline replies and safer draft editing

- Replies use a small inline editor with recipients and reply controls, without
  the full compose-window header. Previous mail sits in a collapsed read-only
  section; choose whether to include it when sending.
- Compose slides up to open and down to close. Expansion reveals final-size
  content without scaling text. Reduced motion is still respected.
- Send waits for an active save or pasted-image import, then sends the same Gmail
  draft. A confirmed send cancels autosave and ignores late file/clipboard callbacks.
- Removable recipient chips, recent-recipient completion, numbered/nested lists,
  and Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y with formatting, font and link restoration.
- Compare a conflicting draft with Gmail in a read-only view before saving a
  separate copy. New Luma rich drafts keep quoted history outside editable text.
- Photo lookups resume when avatars become visible; Settings shows lookup status
  and retry. Available contact pictures and the connected account's own Google
  photo use supported APIs. Google does not expose every Gmail sender picture.
- Rich mail now includes both text/plain and text/html alternatives.

Quit the old version from its tray menu before launching this replacement.
Sign in with Google remains built in; no JSON import or new OAuth scope.
Google verification is still under review; its warning and user cap remain.

Online-first storage is unchanged: server drafts, remote search, paged metadata,
bounded optional body cache and no mailbox/attachment mirror. Rich editing
history is RAM-only, capped at 4 MiB and 100 states (1 MiB per state). Recipient
chips are also memory-only. Contact pictures remain bounded RAM thumbnails.
Installation libraries and temporary graphics/runtime metadata are reported
separately from the configurable mail-body budget.

This is early access. Target Linux/two-account acceptance, sustained mailbox
profiling, remaining roadmap features and Google's decision still block 1.0.
