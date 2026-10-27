# Fluid frontend migration

The 0.9 release is the preserved reference. The replacement uses a custom Qt
Quick scene graph and QML, with the existing Rust provider/cache/security engine.
The frontend communicates with one private Rust child through anonymous pipes;
credentials never enter the UI, and no network listener or disk mailbox is added.
GTK is removed from distribution only after the parity gates below pass.

## Existing wiring to preserve

| Existing view/callbacks | Existing backend | Replacement ownership |
| --- | --- | --- |
| ui_navigation / folder/account disclosure / account shortcuts | UnifiedPager, Gmail::list, Settings accounts | Rust view generation + QML Sidebar |
| ui_rows / virtual_inbox / Ctrl/Shift/select-all / context menu | Conversation, ThreadKey, bounded loaded selection | QAbstractListModel + Rust selection/action guards |
| ui_actions::perform / undo / per-message actions | act_confirmed, MailAction::undo_for, matches_view, matching_label | Serialized Rust mutation service + row reflow events |
| ui::open_thread / expanded message / retry | thread_metadata, message, Cache | Generation-checked Rust reader + morph projection |
| rendering / message_images / attachments | html_mail, mail_images, preview, attachment_save | Off-record reader, RAM image/media sources, explicit downloads |
| compose::open / mailto / reply / forward / identities | Compose, send_as, rich_draft, composer::mime | Qt rich document + Rust compose sessions |
| compose::persist / draft reopen / conflict / discard | edit_draft, save_draft, send_draft, draft_merge, discard_thread_drafts | Per-session Rust save/send lock, revision and uncertain-send guards |
| compose_close / compose_history / recipients / source / find | DraftMode, local_compose recent recipients, text_search | Bounded native document history + close policy + Rust suggestions |
| compose_clipboard / attach / progress / cancel | clipboard_image, compose_drive, drive | In-memory inputs, existing upload/share pipeline |
| signatures / signature_rich | SignatureStore, signature_images, signatures | Rich editor and online signature transport; explicit public-upload consent |
| signin / reconnect / optional scope buttons | Authorization, TokenManager, SystemVault, returned Permissions | Rust OAuth lifecycle + one native consent dialog |
| settings / account CRUD / cache controls / sender_controls | Paths, Settings, Cache, sender_rules | Rust settings commands + custom Settings sheet |
| contact_photos | Photos::get/own, existing bounded lookup rules | Visible-row lookup queue + bounded RAM image provider |
| notifications / tray / desktop / theme monitor | Monitor, Settings, Palette | Rust poll scheduler + native Qt OS adapters and file events |
| resources / close-to-tray / keyboard / scaling | Existing bounded pages, cache policy and close policy | Event-driven lifetime/reclamation; finite springs and reduced motion |

No list action fetches full bodies. Page size remains 25 and loaded records stay
capped at 1,000. The existing Cache implementation, allocation accounting, TTL,
LRU, zero-budget removal and per-account controls remain authoritative. Drafts
stay in Gmail; optional recent recipients keep their existing 100/64-KiB/90-day
bounds. Reader content, document history, image sources and pipe queues are
bounded in RAM. No renderer disk cache, email archive, index or autosave is added.

## Acceptance gates

Track each group through implemented, native-tested, live-accepted and packaged
states. A mapped command or a visible button is not parity. Exercise every group
above, rapid retargeting, failure/rollback, two-account routing, native rich-text
keyboard semantics, HTML privacy, actual tray/notification transport and zero
cache before removing the reference UI. Measure the entire process tree and
installed/runtime bytes; spring support is not a 60-fps performance guarantee.

## Current checkpoint: fluid foundation (not a release)

- Implemented: private Rust engine, existing registration/vault/core services,
  bounded list/page requests, generation-checked reader, confirmed mail actions
  and undo routes, account/settings/OAuth/label commands, unchanged disk cache.
- Native scene: custom vector controls, Noctalia/system palette, pinned rail,
  collapsed unified folders, account disclosure, recycled compact rows, selection,
  row-to-reader projection and reverse, adaptive panes, composer bounds morph,
  compact/fullscreen interruptions, close slide, inline geometry, settings sheet.
- Validation: real native-window synthetic tests exercise the projection and its
  handoff, rapid retargeting, pane recovery, selection, reduced motion and composer
  geometry. Rust core/protocol/cache tests remain separate from live acceptance.
- Composer checkpoint: RAM-only sessions, serialized Gmail saves/sending,
  revision/conflict/uncertain-delivery guards, existing-draft reopen, explicit
  conversion of unsupported HTML, verified sender choices, online signature
  inclusion, close-time draft policy and quit coordination. Native rich text has
  selection-aware fonts/colours/lists/links, bounded undo and find/replace. Replies
  keep quoted history outside authored input; file attachments and private Drive
  upload/progress use the existing core. Input/attachment events never autosave.
  These paths have synthetic native/transport tests, not live Gmail acceptance.
- Restored: account/verified-alias selection with sender-load ordering guards,
  account-scoped recent completion (Tab/Enter and quoted display names),
  clipboard PNG/bitmap attachments through an acknowledged RAM-only pipe,
  queued-Send cancellation on paste failure, scrolling Settings, draft policies,
  recent-recipient opt-out/clear and appearance choices. Native tests seed only
  synthetic recipient metadata and do not modify the OS clipboard.
- Supported photos: visible-avatar subscriptions, two cancellable lookup workers,
  twelve lookups/minute, 64 queued requests and 128 decoded 96-pixel images
  (4.5 MiB pixels maximum) in RAM. No address-book mirror or picture disk cache.
  Settings exposes connection, opt-out and retry. Google Contacts and connected
  own-account pictures are supported; arbitrary private Gmail pictures are not
  exposed by Google's public APIs. Native synthetic display/reset tests pass;
  real two-account acceptance remains pending.
- HTML checkpoint: GTK-free WebView2/Qt WebEngine adapters, theme/original colours,
  full measured newsletter height, approved-image path and bounded viewport
  surfaces. Settings/modal masking, resize and renderer release pass Windows
  synthetic checks. Private runtime cleanup protects live owners and recovers
  stale sessions. Linux and Windows migration CI passed at `d5362d3`; live-account
  acceptance and final resource audits are still required.
- Incremental sync: one sleeping scheduler preserves the existing polling
  preference and core history cursor/dedup/burst limits. It fetches metadata for
  affected conversations and merges rows/reader headers without a view reset or
  automatic loading bar. Manual writes/read receipts guard reconciliation races.
  New replies remain unread and wait for explicit body opening. Windows native
  checks cover reader preservation, row movement, moved selection anchors and
  safe received-link Ctrl clicks. The native model keeps its 1,000-row limit and
  uses direct key lookup. Noctalia watches both file replacement and in-place edits.
- System notifications: Linux calls the Freedesktop notification daemon directly
  over D-Bus; it has no in-app balloon fallback. Windows uses its native tray
  notification transport. Privacy, per-account notification and tray preferences
  are wired. Linux daemon transport has an isolated private-bus CI test; desktop
  notification activation/live acceptance is still required.
- Attachment checkpoint: received/compose cards, visible-only 128×96 image
  thumbnails (32 images/1.5 MiB pixels maximum), a single explicit RAM preview
  and native audio/video playback. DOCX text is read without extraction. Media
  input is capped at 24 MiB and transferred in acknowledged 256-KiB chunks;
  closing releases the player, video frame, image and transfer buffer. Save all
  exports sequentially only to a user-selected folder, with progress, cooperative
  cancellation and collision-safe filenames. Input/preview/export callbacks
  never save a draft or populate an attachment disk cache. Hiding/minimizing the
  window stops springs/photo work and unloads newsletter renderers; showing it
  restores the existing opened content. Final process-tree profiling is pending.
- Pending: three-way conflict-resolution UI, full
  visual signature editing, mailto and remaining rich-text keyboard flows,
  remaining search/label/settings/context/keyboard flows,
  lifecycle reclamation, two-account acceptance, Linux packaging and resource
  audits. The preview has incomplete controls and must not replace 0.9 yet.

See [fluid-validation.md](fluid-validation.md) for reproducible checks. Keep the
reference target until the pending groups have real functional parity; passing
motion checks alone never authorizes removing a working feature.
