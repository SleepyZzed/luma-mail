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

The planned Linux HTML adapter uses off-the-record Qt WebEngine, with memory cache and persistent
cookies/storage disabled and direct network/file/navigation requests blocked.
The planned Windows adapter retains the already-used WebView2 engine through a small GTK-free Rust
native host, because the MinGW Qt distribution does not include Qt WebEngine.
The browser is created only for real HTML layouts. Plain mail uses native text.
Native rich composition will use Qt text documents instead of a per-composer browser.
These adapters are parity work still to implement; the current preview displays
plain text and does not substitute it for HTML in a release.

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
