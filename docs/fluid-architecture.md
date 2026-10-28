# Fluid implementation boundary

Qt Quick/QML owns custom controls, adaptive panes and finite interruptible
springs. Geometry uses an exact damped harmonic solution on Qt's shared animation
clock; retargeting preserves position and velocity. There is no permanent spring
timer and text is rendered at its current font size rather than scaling snapshots.
The frontend is a small native C++ host, built with Qt 6.8 or later.
Mail state and provider work remain Rust. The private luma-engine process uses
anonymous inherited pipes with framed JSON: 4 MiB command bound, 16 MiB aggregate
outgoing queue, 32 active tasks and the existing HTTP concurrency/quota limits.
Tokens remain exclusively in Rust/OS vaults. There is no local HTTP server.

Qt scene-graph rendering can stop at rest. Animations use position/bounds springs,
never scaled text snapshots; reduced motion goes directly to the final state.
Loaded conversation metadata remains capped at 1,000, rendered through a recycled
QAbstractListModel/ListView. Removal follows server confirmation; state changes
never replace the entire inbox after an individual operation.

The Linux HTML adapter uses off-the-record Qt WebEngine, with HTTP cache and persistent
cookies/storage disabled and direct network/file/navigation requests blocked.
The Windows adapter retains the already-used WebView2 engine through a small GTK-free Rust
native host, because the MinGW Qt distribution does not include Qt WebEngine.
The browser is created only for real HTML layouts. Plain mail uses native text.
Native rich composition uses Qt text documents instead of a per-composer browser.
The native rich document is now wired to RAM-only Rust composer sessions. It
blocks all text-document network/file resource loading, caps authored input at
1 MiB/250,000 native characters and undo snapshots at 64/1 MiB. Retained MIME
attachments keep the existing 16-MiB limit. File attachments stream through the
existing private Drive path. Clipboard images now travel in acknowledged
256-KiB chunks under the same aggregate 16-MiB retained-attachment budget. Native
bitmap encoding runs off the UI thread with bounded output and cancellation;
the engine validates the raster before attaching it. Save/Send waits for input
and sender settings; failed/cancelled input cancels queued Send. No image spool,
body resource or clipboard autosave is created.
Quoted history and online signatures are separate from authored reply text and
round-trip in Gmail's MIME draft, without local sidecars. Save/send operations
serialize, reuse one draft ID and do not retry uncertain writes. Quitting uses
the same close policy and waits for writes; only then may the host stop the child.
The HTML adapter preserves sanitized newsletter tables, full measured height,
theme/original colour choices and explicitly approved external images. Width
changes trigger finite measurement; there is no frame callback polling width.
Only the visible vertical slice receives a compositor surface. Scrolling beyond
an HTML body or closing the reader releases its renderer. Windows native child
regions exclude the floating composer and hide for Settings/menus/modal editors.
These paths have Windows native synthetic coverage; Linux CI and live acceptance
are separate gates. Attachment previews/media use the bounded RAM-only adapter described below.

One message's inert PNG image map retains at most 4 MiB of encoded keys/data in
RAM and clears on reader changes. The native message model retains at most eight
opened bodies within a 16-MiB UTF-16 text/document budget (a current oversized
body is bounded by the protocol). It drops older bodies and releases all bodies
on reader close. Approved images are still anonymous, size-limited and decoded/
re-encoded through core, with no direct renderer network or file access.

WebView2 creates temporary runtime scaffolding even in private mode. It is kept
in process-owned `luma-webview-*` temporary directories with active-owner locks,
finite cleanup retries after renderer closure, final exit cleanup and stale-owner
recovery after a crash. A synthetic audit found zero browsing-history entries
and no test message content. This is not a mail cache; final installed-app and
crash/long-duration resource audits remain required.

The Rust Cache implementation and its on-disk envelope are retained verbatim.
In particular, full messages with inline attachment data must not enter the body
cache. Drafts, conflict copies and signatures stay online. Local recent-recipient
bounds and existing user cache preferences remain unchanged. Media input and
preview/image delivery remain memory-only. Explicit downloads are the only
attachment files written, to user-selected destinations.

The current GTK target is temporarily retained as a migration reference, not a
fallback inside the new executable. Remove it and its build/package dependencies
after every row in ui-migration.md has native and packaged parity evidence.
No new release claims 100% parity or a 60-fps/RAM result before those tests run.

The current selected-reader store keeps at most eight full messages within an
8-MiB accounting of payload/metadata allocations, plus bounded map bookkeeping.
It uses LRU eviction, shares attachment payloads through Arc, does not retain a
message larger than that budget, and clears on changing/closing the reader.
The frontend/HTML/media lifetime budgets must be audited separately once those
adapters are wired; this is not a total process-memory claim.

Header, sender, subject, snippet and status labels explicitly use plain-text
rendering. Only the dedicated message renderer may interpret sanitized HTML.
Qt's QML disk cache and automatic shader disk cache are disabled before startup;
compiled QML and prebuilt shader bytecode ship inside the executable. This avoids
introducing a second application cache outside the existing Rust mail budget.
OS/graphics-driver storage and the complete renderer process tree still require the
full final storage audit.

The polling scheduler sleeps between checks at the existing configured interval
(30..3600 seconds, normally 60). It reuses core's history baseline, bounded burst
handling and 512-ID notification dedup. Four account jobs run at most in parallel;
only changed conversation metadata is reconciled. A 128-key pending queue retries
transient/racing updates without fetching full messages or replacing the view.
Provider writes/read receipts and view generations guard stale metadata.
Qt merges and moves individual rows with direct key lookup; incoming headers
preserve already opened bodies and leave new replies collapsed/unread.

Linux notifications use asynchronous `org.freedesktop.Notifications.Notify`
calls, bounded callback metadata and daemon activation signals. Sender-controlled
body text is escaped, privacy preferences apply before transport, and no Qt
balloon fallback is invoked on Linux. Windows uses its native tray transport.
Native plain readers format validated UTF-16 link spans in a resource-blocked
text document, so ordinary linked text does not create another web renderer.

Native desktop activation uses a per-user local socket/named pipe and owner lock,
separate from the Rust engine's inherited anonymous pipes. It accepts only one
bounded mailto URI or a show-window request, uses OS user-only access, and recovers
only its own stale endpoint. Incoming mail links wait in RAM for an enabled
account; a link never replaces an active composer or invokes attachment/send
parameters. No TCP listener or local mail index is introduced.

Save/send completion retains at most eight distinct thread-ID hints for metadata
reconciliation. Both ends of a provider write advance the reconciliation fence;
a read started during a write cannot replace the confirmed result afterward.
Advanced search keeps its enabled-account subset across manual refresh and raw
query edits; cancelling its builder fences late results. Account row reconciliation
uses stable identities instead of resetting the sidebar for preference changes.

Conflict comparison holds only one explicit in-memory review, with bounded plain
previews and the existing 16-MiB attachment editing cap. Cancelling/closing drops
it. Resolution creates a separate unsaved composer with a new Message-ID and
explicit field/attachment choices. It never writes or deletes a Gmail draft by
itself. The original remains online. Unsupported merged HTML retains source mode
until the user explicitly converts it.

The online signature editor uses the existing provider-neutral SignatureStore,
with a single server baseline held in RAM and a native text document created
only while its sheet is open. Initial HTML is retained verbatim until explicit
visual conversion/editing. Save checks the current Gmail baseline; typing never
persists. Hosted-image loading is explicit, anonymous and cancellable. Native
previews accept at most eight approved HTTPS images, each decoded no larger than
256x192 (1.5 MiB pixels total), without direct document network/file loading.
Local pictures are optimized in RAM and uploaded through core only after public
link visibility is confirmed. No signature or picture sidecar is written. Quit
waits for active operations and asks about unsaved signature edits before proceeding
to the normal Gmail-draft close policy.

Linux's browser profile is now acquired by visible HTML adapters and retired
one second after the last adapter closes, rather than constructed at startup.
Its privacy configuration is unchanged. The memory effect is an experiment
pending Linux process-tree measurements; the baseline software-rendered CI
measurement misses the resource goals. Per-process counters accompany aggregate
PSS so UI/engine/renderer costs can be distinguished.


The active conversation has a 32-header native window, separate from the loaded
inbox's 1,000-conversation cap. Header paging uses the single selected thread's
already fetched metadata. An explicitly requested page opens only its last message;
other bodies and attachments are not preloaded.
Its stable end-message anchor protects explicit older-message reading during sync.
A reading epoch plus monotonic header-window revision fences stale replies/pages.
Only headers in the current window receive native mutation projections. The
existing eight-message/8-MiB engine body budget and eight-body/16-MiB UTF-16 native
budget are unchanged. Collapsed body documents are lazy, with spring-close teardown.


The optional native composer window reparents the same document and session; its
lifetime is bounded to one window, destroyed after close/redock. Main/auxiliary
visibility is aggregated only for foreground activity; avatar/HTML ownership also
uses their actual host window. Native window-state tracking preserves restore
state. Closed documents are cleared after their bounds springs finish, and a
rapid reopen cancels cleanup. A single-shot five-second hidden-window grace timer
requests Qt graphics/scene release; no recurring memory trim or mail-state reset
is introduced. Saved window dimensions and density remain small preferences.


Authorization retains one request/URL in native RAM, with a strict official-Google
HTTPS launcher boundary, explicit clipboard action and eight cancelled request-ID
fences. Token exchange, PKCE verifier and vault operations remain entirely in Rust.
Cancellation and credential commit synchronize through the engine job lock; an
owner request ID fences the busy state. Shutdown interrupts reads, requests export
cancellation and waits for accepted writes before dropping the process/writer.
The authorization fixture is opt-in only under isolated design preview, uses an
invalid synthetic client, and never contacts Google or writes vault credentials.
