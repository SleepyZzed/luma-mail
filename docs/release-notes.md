Luma Mail 0.7.0 early access — a rebuilt visual layer and safer draft workflows

- 52 consistent app-owned vector icons replace theme-dependent mail, navigation,
  editing and attachment icons. Inputs have stable focus outlines, Subject is
  compact, menus are quieter, and disclosure arrows are small and borderless.
  The existing pane layout, features, theme integration and scaling are kept.
- Linux main/compose windows stay resizable and undecorated. Luma draws no resize
  border; Super/Meta + right-click is left to the compositor. Hyprland's actual
  configured resize binding still needs a target-desktop test.
- Inline reply X now closes a saved reply. Repeated Reply preserves the same
  inline editor and text; opening/restoring a reply reveals it in the conversation.
- Right-click a draft to Delete draft, with confirmation. Only verified Gmail
  draft resources are discarded; other mail in the conversation is kept.
  Close the composer first to prevent autosave recreating a deleted draft.
- Verified Gmail Send-as addresses appear in From, including inline replies.
  Choosing an alias keeps the owning account's tokens, draft and Drive routing.
  Unavailable aliases block saving/sending until verified or changed explicitly.
- Draft conflict comparison combines independent field changes and requires
  choices for conflicting fields/body and attachment sets. Open merged copy
  creates a separate draft with a new message identity, preserving Gmail's
  original. Comparison never sends mail.
- Available own-account photos can be reused when that sender is another opted-in
  account connected to Luma. Contacts and connected-account photos use supported
  APIs; Google does not expose every sender picture shown in Gmail.

Quit the old version through its tray menu before launching the replacement.
Sign in with Google stays built in; no JSON import or additional OAuth scope.
Google verification remains under review, with its warning and user cap.

The app stays online-first: server drafts, remote search, paged metadata and
bounded optional body caching. Alias lists and merge comparisons are RAM-only.
No mailbox mirror, local draft archive, avatar store, icon download or background
animation loop was added. Package libraries and temporary renderer metadata are
audited separately from the configured body-cache budget.

This is early access. Target Linux/two-account acceptance, sustained profiling,
remaining roadmap features and Google's decision still block 1.0.

