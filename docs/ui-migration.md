# Fluid frontend migration

The 0.9 release is the preserved reference. The replacement uses a custom Qt
Quick scene graph and QML, with the existing Rust provider/cache/security engine.
The frontend communicates with one private Rust child through anonymous pipes;
credentials never enter the UI, and no network listener or disk mailbox is added.
GTK is absent from the preview packages. The legacy source remains a parity
reference until the final acceptance gates below pass.

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
- Labels/actions checkpoint: on-demand account-scoped labels with recycled
  navigation and picker rows, create/rename/delete confirmation and a shared
  themed action menu. Bulk menus hide sender/reply actions, toggle stars and
  importance from the actual selection, and omit Move to Inbox inside Inbox.
  Dedicated draft deletion uses core's exact draft lookup/revision guards and
  keeps sent/received messages. Existing server-side sender filters are wired
  through confirmation, optional Google connection and Settings unblock controls.
- Account/cache controls: enable/disable, default sender, rename/reorder/removal,
  reconnect and optional feature connections; custom/zero cache, expiry, per-account
  allocated usage/clear and exit cleanup. Settings writes are serialized; deleting
  an account reloads the appropriate view and drops its label metadata. Account
  name/notification changes no longer reset photo caches or token managers, and
  unchanged palettes no longer rerender opened newsletters. One allocation report
  scans cache headers once for all account totals, without a new index/sidecar.
- Search and desktop links: advanced Gmail operators, validated dates, account
  subsets that survive refresh/raw-query edits, cancellation of stale builders,
  and bounded RFC 6068 mailto input. Plus signs are preserved; link parameters
  cannot attach local files, save or send. A per-user native local socket/pipe
  forwards a second launch to the existing window, with bounded frames, a process
  lock and stale-owner recovery. It has no TCP listener and carries no tokens.
- Draft comparison: six-field three-way resolution, separate read-only previews,
  explicit body/attachment choices, revision/cancel guards and a fresh unsaved
  merged copy. Gmail's original draft is untouched. Closed comparison/editor
  view models release their content; Save/Send hint only the affected thread IDs
  to the bounded metadata reconciler for immediate Drafts/Sent updates.
- Per-message controls now target one message without modifying sibling labels;
  reader flags merge immediately without replacing bodies or attachment strips.
  Account rows reconcile in place. Internal pane handles and keyboard resizing
  retain the user's bounded layout preferences, alongside search/account/action
  shortcuts and input-safe Delete/Select all behavior. Tray activation restores
  minimized windows and preserves maximized state.
- Renderer auditing found a Windows reentrancy defect: native creation can pump
  geometry callbacks before returning. Creation is now guarded against duplicate
  controllers, and inactive readers explicitly release their controller. Native
  checks assert one live controller and zero after hide/close; process-tree
  resource measurements are a separate gate.
- Online signatures now have a lazy native editor: installed fonts, colours,
  lists, paragraph formatting, tables, hosted pictures and explicit public-Drive
  image-copy approval. Source remains intact until visual editing is selected.
  Provider-neutral SignatureStore guards the Gmail baseline; typing never saves.
  Close/quit coordinates save/discard, and closing destroys the document, previews
  and bounded undo history. Windows native and private-pipe synthetic tests pass;
  real Gmail/recipient-image acceptance remains required.
- Long conversations: 32 native headers per explicit page, latest-message reveal,
  stable historical anchors, revision fences and lazy collapsed body documents.
  All older headers remain accessible. A requested page opens its last message;
  other bodies/attachments are not preloaded. Existing body/cache budgets remain.
  Native 2,503-message acceptance verifies bounded delegates and complete release.
- Native pop-out/dock now reparents one editor/session, preserving formatting,
  selection and attachments. Platform decorations, minimized restore and the
  ordinary draft close policy remain intact. Native Windows checks pass.
- Close cleanup waits for spring completion; a five-second hidden-window grace
  period releases recreatable graphics resources without polling or disk writes.
  Compact rows, saved main-window size and visible unified-account badges use the
  existing preferences and metadata. Linux memory verification remains pending.
- Sign-in recovery now has a spring sheet with the real request lifecycle,
  system-browser retry, explicit copy link and cancellation. Empty-account
  onboarding requests the existing optional connections in one Google flow.
  Cancel/Quit cannot interrupt an accepted credential commit; shutdown also waits
  for accepted provider writes. Browser/clipboard-isolated native tests pass.
- Pending: remaining rich-text/forwarding and layout flows, final
  lifecycle/resource hardening, two-account acceptance,
  Linux packaging and sustained resource audits. The preview has incomplete
  controls and must not replace 0.9 yet.

See [fluid-validation.md](fluid-validation.md) for reproducible checks. Keep the
reference target until the pending groups have real functional parity; passing
motion checks alone never authorizes removing a working feature.


File drops, bounded retryable RAM-only Undo and inline message/HTML error recovery
are implemented. The narrow/collapsed sidebar footer no longer overlaps. Fourteen
native scene groups passed in this continuation (254 checks); latest reader,
HTML, long-thread and sync tests pass. See fluid-validation.md. GTK-free packages,
installed/runtime notices and byte audits, live Gmail/Hyprland acceptance, inline
image authoring, whole-conversation forwarding and Linux RAM goals remain open.

- Portable checkpoint: optimized Windows bundle passes all 254 native checks with
  SDK paths removed. Direct Basic imports keep the custom design and omit unused
  theme runtimes. Linux AppImage/native Wayland/WebEngine deployment and exact
  source/notices checks are now wired into CI; successful Linux packaging is still
  pending. The fixed installation is audited separately from cache/runtime data.
  Qt 6.11.2 is used for the forthcoming Linux packaging checks. See fluid-packaging.md.


- Composer table tools now expose the existing native rich-table model, including
  Tab/Shift+Tab cell navigation, capped row growth, final-cell exit and popup
  cleanup. Fourteen local native groups pass 271 checks. Lazy VideoOutput creation
  preserves tested playback while avoiding eager media-backend startup. Linux
  37c8178 portable checks pass; Windows startup/packaging and RAM/desktop gates
  remain open. Exact evidence is recorded in fluid-validation.md.


- On-demand composer controls now release the native editor/document/history
  after closing springs settle; rapid reopening and pop-out retain the existing
  editor. The latest local suite passes 272 checks. Both e27b4fd preview packages
  pass without SDK paths (271 Windows/268 Linux), with clean Windows source
  provenance. An isolated zero-cache live read also passes. Newer lifetime changes
  still require matching package/desktop acceptance; GTK reference removal and
  1.0 remain gated.


The GTK-free 0.10 preview is published as v0.10.0-dev.1 at d4889b7. Both exact
packages pass (269 Linux/272 Windows); all eight uploaded asset digests and the
release tag/source archive commit were verified. The 0.9 tag/source remains the
reference; 1.0 and target-desktop acceptance are still open.
