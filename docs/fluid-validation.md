# Fluid migration validation

This branch is an implementation checkpoint, not a distributable replacement or
1.0 claim. The original GTK executable remains the feature reference. The new
native host and Rust engine do not depend on GTK.

## Reproduce

Build the existing Rust core and private engine with the normal Rust toolchain:

```sh
cargo test -p luma-core -p luma-engine
cargo clippy -p luma-engine --all-targets -- -D warnings
cargo build -p luma-engine
```

The configured production engine will use the same maintainer-supplied build-time
Google registration and `--features require-google-client`. No runtime JSON
import or new credential format is introduced. Synthetic preview checks require
no Google client or account.

Install Qt 6.8 or newer with Quick/Declarative, QuickControls2, Widgets, SVG,
Multimedia, ShaderTools, Test and (Linux) DBus, then use CMake/Ninja:

```sh
cmake -S crates/fluid -B build-fluid -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build-fluid
ctest --test-dir build-fluid --output-on-failure
LUMA_ENGINE="$PWD/target/debug/luma-engine" \
  LUMA_MOTION_REPORT="$PWD/fluid-motion.json" \
  build-fluid/luma-mail-fluid --design-preview --motion-test
```

On Windows use the corresponding `.exe` paths and environment-variable syntax.
Build `luma-webview` too, then run `python packaging/webview2-loader.py --binary
target/debug/luma-engine.exe` to stage the pinned loader required by the GNU
Windows adapter. Repeat for the release directory before packaging.
The host creates a temporary isolated preview root unless `LUMA_TEST_ROOT` is
explicitly supplied. It uses only clearly synthetic Alex Morgan/example.com mail.
The end-to-end motion test must run in an exposed native window; a hidden or
headless window cannot establish observed frame delivery. Linux CI uses Xvfb
for functional coverage, which does not establish Hyprland/GPU performance.

The independent spring tests check initial layout, position/velocity continuity
on interruption, convergence, reduced motion, invalid input and no further value
updates at rest. The native test checks actual QML geometry and real pipe events:
opening/replacing/closing a reader, exact avatar handoff, compact inbox, retained
account folders, range selection, reduced motion, row removal, compact compose,
interrupted maximize/restore, close slide and no repeated frame requests at rest.
The bounded diagnostic interval sample is accompanied by an independent total
frame counter, so reaching the sample cap cannot make the idle check pass.

Run `--design-preview --composer-test` separately with `LUMA_COMPOSER_REPORT`
set to a JSON path and `LUMA_PREVIEW_TWO_ACCOUNTS=1`. Its 28 native checks exercise the actual form/pipe session,
To-field focus, Tab/Shift-Tab, selected rich formatting, keep/close policies,
quit cancellation, input release, inline reply docking, separated quoted history
and repeated Reply without maximizing, chunked PNG/bitmap paste, account switching
and recipient completion. It never writes/sends real Gmail mail. The test uses
an isolated synthetic MIME source rather than replacing the user's OS clipboard;
only its optional recipient-metadata fixture is written inside the test root.
Transport mock tests separately cover serialized saves, latest input during a
save, one-draft reuse, duplicate-send prevention, conflicts and uncertain writes.
The native document suite covers logical font scaling, paragraph lists, safe
links, Unicode legacy formatting, bounded undo and blocked resource loading.

## Local evidence

27 October 2026, Windows, Qt 6.12/UCRT64: CMake release build and the spring suite
passed. The real native-window motion and composer suites passed on synthetic
data. Core tests: 126 passed and one isolated OS-vault test intentionally ignored;
engine tests at the attachment checkpoint: 24 unit tests and eight real-pipe tests passed. Strict Clippy is a
separate required check. Exact run details are in the diagnostic JSON artifact.

The raw frame intervals describe this display/run only. They are not a universal
60-fps guarantee, a long-duration CPU profile, a Linux result, or an acceptance
result for unfinished HTML/compose/provider functionality. Re-run the complete
process-tree and disk audits after parity is implemented. No current resource
claim replaces the documented outstanding Linux RAM targets.

The first Linux CI scene/spring run also passed at commit `061284a6`. Windows CI
exposed two test assumptions about desktop width; the assertions now derive the
expected split/expanded bounds from the actual window, including the narrow
reading mode. Diagnostics record window dimensions. This checks responsive
behavior rather than requiring a particular runner monitor resolution.

At `5f4dadb7`, the Linux and Windows native scene jobs, spring tests and engine
checks passed in [run 40551772145](https://github.com/SleepyZzed/luma-mail/actions/runs/40551772145).
The next check also verifies literal header rendering; the shell explicitly
turns off Qt's QML/shader disk caches. These safeguards do not change the Rust
mail-cache preference or its budget.

The composer checkpoint also recovered cleanly after the interrupted laptop
session: Git connectivity, saved working tree and prior platform CI were checked
before changes resumed. New draft/input operations create no local draft file;
the real-pipe zero-cache checks verify no local draft file or body-cache directory
is written. Optional recipient metadata is tested separately. This does not
replace the final installed-app/process-tree audit.

The photo/HTML checkpoint adds native `--photo-test` (five checks) and
`--html-test` (ten checks including hide/restore), using `LUMA_PREVIEW_PHOTOS=1` or
`LUMA_PREVIEW_HTML=1` and `LUMA_PROFILE_REPORT` / `LUMA_HTML_REPORT` JSON paths.
On Windows, set `LUMA_WEBVIEW` to the built `luma_webview.dll`; its adjacent
`WebView2Loader.dll` must be present. `cargo build -p luma-webview` builds this
GTK-free adapter. Linux requires Qt WebEngine/Quick 6.8 or newer. The reader test
uses only a synthetic newsletter and blocked example.invalid image; it does not
contact a sender or send mail. All four native Windows scene suites pass after
the avatar/reader wiring. CTest also covers image budgets and runtime ownership/
cleanup; active-owner directories must survive stale-session recovery.

The synthetic WebView2 audit recorded 808,765 bytes of temporary runtime metadata,
63 files, zero History URL rows and no test-body marker. One earlier test folder
outside the workspace could not be removed because automatic approval rejected
that deletion; it was left intact. The replacement runtime has finite cleanup
and stale-session recovery, with successful native cleanup checks. No final
whole-application RAM/CPU/storage claim is made from these fixture tests.

At `d5362d3`, Linux Qt 6.8 and Windows Qt 6.12 native migration checks passed in
[run 40592681875](https://github.com/SleepyZzed/luma-mail/actions/runs/40592681875),
including the photo/HTML adapters. The next checkpoint adds `--sync-test` with
`LUMA_SYNC_REPORT`: twelve native checks exercise incremental row movement,
unchanged reader content, selection-anchor tracking, unread/collapsed incoming
replies and exactly one Ctrl-click link launch. The URL launcher is intercepted
inside the test process; it never opens a real browser. The incremental event
fixture is separate from core's real HTTP history/burst tests. Linux CTest also
runs a fake notification daemon inside `dbus-run-session`, validating actual
D-Bus transport and thread activation without contacting the user's daemon.


The attachment checkpoint adds `--design-preview --attachment-test` with
`LUMA_ATTACHMENT_REPORT`. Its fourteen native checks exercise visible composer
thumbnails, the 1.5-MiB pixel bound, card-to-preview bounds morph, plain-text
previews, chunked audio seek/playback, decoded video frames, close-time resource
release and no local mail/draft cache. WAV and MPEG-4 fixtures are synthetic;
only their explicit source files are written inside the isolated input folder.
Linux CI supplies a null audio sink so playback is tested without speakers.
The real-pipe integration check also exports a duplicate filename twice and
verifies the original plus both collision-safe copies. The core export test
covers cancellation and path sanitization. A cancelled pending download is
cooperative; normal quit waits for partial-output cleanup.

All six Windows native suites pass at this checkpoint: motion 32, composer 28,
photos five, HTML ten, incremental sync twelve and attachments fourteen checks.
The motion suite now explicitly rejects a hidden/unexposed test window; such a
window cannot provide frame/geometry evidence. Five local CTest suites and
strict Rust Clippy pass. Updated Linux/Windows CI and live acceptance remain
separate gates. Neither the codec nor renderer memory totals are established by
functional fixture checks.


At `f431b69`, all Linux and Windows attachment/scene checks passed in
[run 40601786955](https://github.com/SleepyZzed/luma-mail/actions/runs/40601786955),
including media playback from RAM and the Linux private D-Bus suite.

The next navigation checkpoint adds `--design-preview --navigation-test` and
`LUMA_NAVIGATION_REPORT`. Twenty-four native checks exercise account label
create/rename/delete, a real right-click label hit target, bulk action visibility,
importance reversal, explicit sender-rule confirmation/unblock and the dedicated
draft-delete endpoint. Repeated QML objects are found through the visual tree
as well as QObject ownership; treating them as ordinary window children hid a
valid menu in an early test. The synthetic rules never touch a Google account.

The Windows navigation, motion, composer, photo, sync, HTML and attachment suites
all pass after the new controls. Strict Clippy and core 126/one ignored, engine
24 unit/ten pipe tests pass. The pipe tests additionally exercise label/rule
scope guards, concurrent preference preservation, account reorder/disable/remove
and zero-cache usage reporting. Core's allocation report matches the existing
total/per-account accounting and adds no disk index. Live Google/desktop and
whole-process resource acceptance remain outstanding.

## 28 October continuation

Current local checks: core 128 passed/one OS-vault test intentionally ignored;
engine 28 unit and 13 real-pipe tests; strict Clippy; six Windows native CTest
suites. The native scene groups pass: motion 33, composer 28, navigation 31,
search 13, draft comparison 13, incremental sync/message actions 16, attachment
14, photos five, and Windows HTML 13 (Linux HTML has ten platform-neutral checks).
The single-instance suite also passed 20 consecutive local runs after fixing
acknowledgement/disconnect and already-buffered input races.

`--design-preview --search-test` needs `LUMA_PREVIEW_TWO_ACCOUNTS=1` and
`LUMA_SEARCH_REPORT`. It validates actual search fields, account-scope persistence,
cancellation and mailto prefilling/Bcc visibility/close-time content release.
`--conflict-test` needs `LUMA_PREVIEW_CONFLICT=1` and `LUMA_CONFLICT_REPORT`.
It uses a synthetic conflicting draft through the actual engine and verifies
field choices, a fresh unsaved copy, original preservation and review release.
Neither path can send or write Gmail mail in design-preview mode.

`--design-preview --resource-test` is an exposed-window lifecycle fixture with
20-second settled phases: baseline, 1,000 loaded records, scrolling, newsletter,
reader release, background hiding and restoration. Phase transitions have a
separate marker so renderer startup is not counted as settled inbox CPU. The
fixture uses single-shot phase boundaries, with no in-app sampling timer during
idle. It asserts row virtualization, complete body release and no orphan Windows
native controllers. Lifecycle assertions alone are not RAM/CPU evidence.

Sample all descendants with:

```sh
LUMA_ENGINE="$PWD/target/debug/luma-engine" LUMA_PREVIEW_HTML=1 \
  python3 packaging/profile-processes.py build-fluid/luma-mail-fluid resources.json --fluid
```

Windows: run `packaging/test-windows-performance.ps1 -Fluid -Binary <exe>
-Evidence <new-output-directory>` with `LUMA_ENGINE`/`LUMA_WEBVIEW` set. It records
private working set, private commit, shared-inclusive working set and per-process
attribution. Linux uses RSS/PSS/private pages from `/proc`; the metrics are not
interchangeable. These short synthetic runs do not establish long-term real sync,
Hyprland/GPU performance, installed-package footprint, or universal RAM targets.
The first Windows audit exposed a duplicate-controller lifetime defect; its
counter assertions now prevent accepting a closed reader that still owns one.

The 28 October Windows release-host/debug-engine synthetic process-tree run
recorded approximately 136 MiB private working set at fresh idle, 150 MiB after
scrolling 1,000 metadata records, 254 MiB with the newsletter open, and 155 MiB
after closing/hiding (154 MiB after restoration). The corrected reader returns
to three application/helper processes, with zero native HTML controllers; the
previous faulty version retained nine processes after closing. The native tree
has at most 23 recycled row widgets for the 1,000 loaded records. Settled CPU
samples rounded to 0.00% of one core at this sampling resolution. These are
Windows private-working-set values, not Linux PSS or an absolute CPU guarantee.
The active HTML target of 200 MiB is not met. Raw per-process counters, separate
private commit/shared-inclusive working set, executable hash and six lifecycle
assertions are in validation/fluid-resource-windows.json and
validation/fluid-resource-lifecycle-windows.json. Linux/real-sync results remain
separate required gates; no memory trimming API or degraded HTML renderer is used
to make the measurement appear lower.

At `fefc8b3`, all fluid Linux/Windows CI jobs passed in run 40617034644. The
Linux scene uses Xvfb plus Openbox so maximize/restore tests run against an actual
window manager. Its complete software-rendered process-tree audit recorded
330.8 MiB idle PSS, 338.8 MiB after scrolling and 420.7 MiB with HTML open;
release returned to 365.3 MiB. Settled CPU was mostly 0.00% at the sample
resolution, with a 0.062% mean during release. These Linux memory goals are not
met. The JSON is in validation/fluid-resource-linux.json; the source artifact is
https://github.com/SleepyZzed/luma-mail/actions/runs/40617034644/artifacts/12785473142.
The next experiment defers creation of the private Linux browser profile until
an actual visible HTML reader needs it, releases it after the last view retires,
and adds per-process counters. Its memory effect is unverified until CI reruns.

The signature checkpoint adds `--design-preview --signature-test` with
`LUMA_SIGNATURE_REPORT`. Fifteen native Windows checks cover lazy creation,
original-source protection, public-image approval, selected colour formatting,
HTTPS picture previews, tables, sanitized preview-only saving, dirty-state updates,
quit/keep/discard coordination and complete editor release. Its preview transport
holds saved HTML only in RAM; it never calls Gmail or uploads a real Drive file.
The engine now passes 28 unit/14 private-pipe tests; document CTest also checks
picture URL/budget isolation, logical image scaling, table navigation and keeping
independent paragraph indentation during alignment/line-spacing changes. The
signature Linux UI checks and profile-lifetime experiment are newly queued CI
gates, not yet live Gmail or target-desktop acceptance.


## Reader headers and ordered cancellation — 28 October continuation

The replacement now exposes per-message From/To/Cc/Bcc/Reply-To/Date/Message-ID
through a spring-opening, selectable details panel. Its extra text document is
created on demand and destroyed after closing. Background metadata reconciliation
also keeps a selected notification/search conversation current outside the active
account subset, without rebuilding a searched inbox or retrieving new bodies.
Reader-epoch fences reject metadata prepared for a previous selection.

CI at 600c089 exposed two further failures: comparison cancellation was dispatched
asynchronously and could overtake a subsequent merge; native X11 visibility events
could lose a maximized restore state. Cancellation now invalidates its ticket in
pipe order, and restore tracks the native window-state signal. Desktop acceptance
waits for a settled, actual state rather than a fixed short delay. Linux process
profiling runs independently after a successful native build so an earlier scene
failure cannot hide its resource evidence. The prior Linux 330.8 MiB PSS result
still stands until that new measurement completes.

Windows PowerShell 5 resource summaries now convert sample hashtables to plain
records before Measure-Object, avoiding a report-generation failure in the
retained reference frontend job. These changes do not add persistent mail data
or change the configured cache/TTL limits.


## Bounded long conversations — 28 October continuation

The selected conversation sends only 32 headers to the native reader at a time.
Earlier/Later/Latest controls preserve access to the complete current thread,
including messages beyond the old generic 1,000-row model limit. Opening an older
message pins that historical window when new replies arrive; Latest is explicit.
Bodies remain on demand. Collapsed message bodies now use a lazy native loader,
retained through the closing spring and released when it settles. Long pages
spring-scroll their last message into view. Reader/window revision fences reject
late metadata and navigation events, and only visible-window flags are projected
after a provider mutation.

The real private-pipe test walks all 2,503 synthetic headers with only one explicitly
opened body per page, and no mail/draft files or remaining-body preload. Windows native acceptance has
17 passing long-thread checks, covering 32 header delegates, one body document,
latest-message visibility, earlier/latest navigation, preserved open bodies during
incoming replies, stale-window rejection and complete close release. The shared
native model test also checks stable identity and eviction of departed bodies.
Rust engine checks now cover 32 unit and 15 real-pipe tests; strict Clippy passes.

The ca64d4f Linux functional suites passed, including online signatures and native
window restoration. Its resource fixture completed but Chromium reported invalid
shared-image buffers under Xvfb/llvmpipe. That run remains invalid for a memory
claim: the older profiler discarded counters on any diagnostic error. Profiling
now writes counters and explicit failure/phase status before failing, preserving
per-process attribution for investigation. The CI-only resource run explicitly
uses Chromium software rendering, following [Qt's documented GPU diagnostic
switch](https://doc.qt.io/qt-6/qtwebengine-debugging.html). This does not change the
application's graphics defaults, disable sandboxing, or establish real Hyprland
GPU performance. Backend configuration is recorded with the report; differences
must not be attributed to lazy profiles alone.


## Complete software-rendered Linux attribution at c4b07ec

All Linux functional scene suites, including 17 long-conversation checks and the
signature editor, passed in [run 40624580688](https://github.com/SleepyZzed/luma-mail/actions/runs/40624580688).
The explicit Chromium-software resource run completed with all six lifecycle
assertions and readable PSS for every descendant process. It measured 258.2 MiB
PSS at baseline (two processes), 278.5 with 1,000 loaded conversations, 295.4 after
scrolling, 452.8 with HTML (five processes), 405.3 after release and 405.5 in the
background (four processes). Settled CPU samples were 0.00% at the one-second
counter resolution. These results fail the requested RAM goals; the changed
software renderer configuration means they cannot isolate the effect of lazy
profiles against the earlier 330.8-MiB baseline.

Per-process samples show the main Qt host dominating idle allocations; after
HTML closes the renderer view is gone, while Chromium framework/zygote processes
and host allocations persist. No mailbox archive was added. The raw aggregate,
per-process and lifecycle evidence is retained in docs/validation. Actual
Hyprland/GPU use and a sustained live-account run remain separate required gates.
The Windows job at that commit failed the old fixed-900-ms document-release
assertion; close teardown is now being tied to spring completion and its native
check waits for actual release rather than assuming a fixed frame deadline.


## Pop-out, close completion and background graphics — 28 October

The native pop-out window reparents the same composer/document/session; it does
not create another draft or editor. Docking restores its former compact, expanded
or inline context. Native close follows the existing draft policy. Windows keeps
system title-bar controls, while Linux delegates its frameless window to the
compositor. Auxiliary-window visibility keeps the active editor responsive when
the inbox is hidden; hidden inbox avatars and HTML still stop independently.
A shared native window-state tracker restores minimized/maximized windows.

Closed-composer teardown now follows the completion of the actual closing springs,
rather than an 800-ms timer. Rapid reopening cancels pending cleanup. The native
check waits for release before continuing. All 12 affected Windows scene groups
pass without QML warnings (219 checks), including 14 pop-out checks, 33 navigation
checks and density changes that preserve loaded rows. Comfortable/compact density,
account-source badges, recipient/input accessibility names and saved main-window
size are wired to existing preferences. No mailbox refresh is needed for density.

After five seconds deliberately hidden/minimized, a one-shot requests release of
recreatable Qt scene/graphics resources. Short hide/restore cycles retain them;
there is no repeated idle trim or working-set manipulation. This follows [Qt's
resource-lifetime API](https://doc.qt.io/qt-6/qquickwindow.html#setPersistentGraphics).
The local Windows resource binary, built before the later density/geometry changes
and identified by its SHA-256 in the raw report, passed all seven lifecycle checks:

| Phase | Complete-tree private working set (MiB) |
| --- | ---: |
| Fresh baseline | 128.8 |
| 1,000 loaded conversations | 140.7 |
| After scrolling | 144.0 |
| Open newsletter | 251.3 |
| Reader released, window visible | 149.9 |
| Background after resource release | 98.6 |
| Restored native scene | 128.6 |

Settled CPU samples were 0.00% at the sampling resolution. Windows private commit
is reported separately (341.2 MiB baseline, 522.1 HTML, 194.7 background); these
figures are not Linux PSS. The newsletter phase still exceeds the requested active
RAM goal, and no Linux result for the background-release change is claimed yet.
The isolated fixture retained only its small settings file, with no cache/body,
attachment or draft directory. Package/long-soak/real-GPU acceptance remains open.


## Sign-in recovery and accepted writes — 28 October continuation

The custom sheet now follows real authorization request IDs, opens the system
browser, reports launch failure and offers explicit retry/copy/cancel. Cancelled
or mismatched callbacks cannot reopen a browser. Only bounded official Google
HTTPS authorization URLs reach that launcher; the URL is cleared on completion,
error or cancellation. Empty mailboxes now offer one-click Google onboarding and
useful empty/search/disabled-account states. Initial sign-in requests the existing
Gmail and optional Contacts/Drive/sender-control connections together; actual
returned permissions remain authoritative.

Cancellation is accepted before credential persistence. The engine fences the
vault/settings commit against cancellation, while quit waits for accepted provider
and credential writes (including read receipts) instead of aborting them. An
old cancelled task cannot clear a newer request's busy state. No token or verifier
enters the QML scene and no credential JSON file is created. The test-only synthetic
authorization fixture has no Google request or vault/configuration write.

Windows native acceptance passes all 13 scene groups without QML warnings
(233 checks), including 14 auth recovery checks with an isolated browser launcher
and clipboard. Engine validation passes 33 unit and 16 real-pipe tests; the auth
cancel/retry/commit/quit case passed 20 consecutive runs. Six CTest groups pass.
These are state/lifetime/security-boundary checks, not real Google consent proof.

At 063ab88, both complete Linux and Windows migration CI passed in
[run 40627979471](https://github.com/SleepyZzed/luma-mail/actions/runs/40627979471).
The Linux software-rendered resource run measured 247.4 MiB PSS baseline, 253.7
with 1,000 loaded records, 255.6 after scrolling, 410.8 with HTML, 355.2 after
reader release, 331.1 backgrounded and 362.0 restored. All seven lifecycle checks
and complete-PSS checks passed. Settled CPU samples were 0.00% at counter resolution.
Releasing the hidden scene helps, but RAM goals remain unmet; render configuration
and CI variability must be preserved when interpreting differences between runs.


## File drops, bounded Undo and reader recovery — 28 October continuation

Compose accepts local file drops through the existing attachment/Drive pipeline.
Drops explicitly select Copy even when the source proposes Move, reject remote
URLs and batches above 64, and never insert paths into authored text or create a
draft. Narrow/collapsed sidebar footer controls remain separately accessible.

Undo consumes confirmed actions individually, retains failed/unattempted inverses
for explicit retry and updates availability independently of a stale mailbox view.
Restored rows check the live generation, account scope, folder and server search
result; open messages receive corrected flags. Its single RAM-only batch is
limited to 8,192 message entries and a conservative 2-MiB allocation budget. An
oversized batch discards the entire inverse and explains why Undo is unavailable;
no partial Undo is offered. Successful Gmail changes still complete. No inverse
is written to disk, and accepted writes finish before application shutdown.

Message-header, body and HTML errors clear their pending state and offer inline
retry without resetting the inbox. Empty bodies are valid; evicted bodies can be
explicitly loaded again. Retries retain the account, reader and historical header
window. The isolated design-preview failure fixture never reaches Google. Its
native test exercises three failures through actual QML buttons and the engine
pipe, verifies successful recovery, then checks reader release.

Fourteen native groups passed in this continuation (254 checks), including 18
attachment, 35 navigation and 15 recovery checks. Reader/HTML/long-thread/sync
scenes were rerun after recovery wiring changed. Six CTest groups, 37 engine unit
and 16 real-pipe tests, strict engine Clippy and the unchanged core suite (128
passing, one Windows platform-vault test ignored) pass. Both platforms passed the
preceding auth/lifecycle checkpoint in run 40631799634. Synthetic acceptance does
not establish live Gmail, Hyprland, final packaging or the unmet Linux RAM goal.


## Portable packaging checkpoint — 28 October continuation

The Windows portable package passes all 254 native scene checks (14 groups)
with developer engine/WebView/Qt/QML paths removed and only Windows system paths
available. It uses the optimized private Rust engine/adapter and compiled Basic
controls with the existing custom styling; unused Qt control themes are no longer
deployed. The executable now embeds the supplied star icon. The launcher, forms,
HTML, media, attachments, recovery, pop-out and motion remain functional.

Local candidate D has explicit dirty-source provenance (`e6d6dc9` plus the current
packaging/UI changes). Its ZIP is 102,134,858 bytes (97.4 MiB), installation logical
size is 275,822,634 bytes (263.0 MiB), and native filesystem-reported allocation is
278,226,848 bytes (265.3 MiB). These are fixed program/framework bytes, separate
from cached emails. Complete shared-library ownership, notices, source links,
Rust dependency/standard-library references, file hashes and archive checksum are
included. The FFmpeg build includes its linked codec dependencies; none are
silently removed at the cost of media support. No account files are copied.

The packaged release-process tree measured private working set of 131.9 MiB idle,
143.3 with 1,000 rows, 145.5 after scrolling, 252.9 with HTML, 151.2 after release,
103.2 backgrounded and 134.2 restored. Settled CPU samples were 0.00% at counter
resolution; private commit and all phase samples are reported separately. All
seven lifecycle checks passed. The active HTML target remains unmet, and these
Windows measurements are not Linux PSS or a live-Gmail soak.

The new Linux packaging path uses pinned linuxdeploy/Qt tools, native Wayland
plugins, WebEngine helper/resources/locales and an explicit relocatable launcher.
Matching Qt/Chromium source notices are streamed from exact upstream archives;
only the notices (9,645,243 bytes for eight Qt 6.11.2 modules) are retained, with
reviewed archive and inventory hashes. It includes distro and Rust source/notices
and an exact Git source archive. CI moves the Linux runtime from Qt 6.8.3 to
6.11.2 before exercising the native and portable suites. AppImage validation is
pending that CI run; no successful Linux package or new release is claimed yet.

At e6d6dc9, engine and Windows CI passed. Linux passed the new attachment,
navigation and recovery checks but caught a maximized tray-restore race. The host
now commits Windowed restore state only after the X11 event settles, and activation
of an already visible window preserves its current geometry. The native test now
waits for matching visibility and window-state acknowledgement. Windows passes;
Linux confirmation is pending. The original GTK release remains the reference.


## Settings lifetime and packaging follow-up

Settings creates its account/forms only on opening and retains them until both
closing springs settle. Closing then destroys the controls; reopening restores
current saved values. Closed-window resizing no longer makes an offscreen
Settings panel temporarily mask the reader. Four new native lifecycle checks
pass (39 navigation checks; 258 checks across the scene groups in this
continuation). Motion, composer, HTML, sync and pop-out checks also pass after
the relevant changes; no QML warning was reported in the lazy-settings runs.

The development Qt host with the same optimized engine/adapter measured Windows
private working set of 124.5 MiB baseline, 134.9 with 1,000 rows, 138.3 after
scrolling, 239.9 with HTML, 139.6 after release, 94.4 backgrounded and 125.0 restored.
This is a separate synthetic run after the Settings change, not a strict paired
experiment against candidate D. Settled CPU samples were 0.00% at resolution,
all seven lifecycle checks passed, and HTML/Linux resource goals remain open.

The first branch packaging run (40637853387) exposed setup gaps: the Windows
release step lacked Cargo in the MSYS shell; Qt's Linux deployer tried to deploy
an unused NMEA/GPS plugin without QtSerialPort. Windows now builds both Qt and
Rust with UCRT GNU, matching the locally tested portable candidate. The Qt deployer
is called directly with explicit GTK-theme/GPS plugin exclusions; email documents
do not request geolocation. Source-notice collection and its pinned 6.11.2 cache
passed in that run. Neither registered preview package is claimed complete yet.

Linux's maximized hide/restore test passed in the branch run but failed in the
parallel PR run. The host now reasserts a remembered hidden window state before
remapping, avoiding Qt's same-state shortcut. A 64-event synthetic trace records
visibility/native/cache changes for diagnosis. Windows sync and pop-out checks
pass; Linux confirmation remains required.


The external-HTML draft toolbar now leaves Attach enabled while disabling only
formatting controls. The form already omits body replacement in source mode, so
adding files/headers preserves the original HTML until explicit conversion. Two
new native checks pass (30 composer checks; 260 scene checks in this continuation).

At bbf5d2c, the branch Linux native suite passed, including the tray-restore
stress test, but packaging stopped at its legacy-library guard. The guard now
reports actual ELF dependency paths in runtime directories, separating source
notices from native libraries. The Windows GNU test build now explicitly stages
the pinned WebView2 loader, includes its Python/inspection tools and saves built
Rust dependencies before GUI tests. These are fresh-build setup corrections;
portable CI and live acceptance remain pending.


## Clean-build dependency corrections

At ecd32bc, Linux native acceptance passed in both the branch and PR runs.
The branch package guard identified `usr/plugins/wayland-decoration-client/libadwaita.so`.
This is Qt's Wayland decoration plugin, distinct from GTK4's `libadwaita-1.so`.
The ELF guard now distinguishes those names and continues rejecting actual GTK
runtimes. Windows packaging reached dependency/notice collection but lacked Git
in the MSYS environment when recording provenance; the environment now explicitly
installs Git. Registered portable acceptance remains pending these corrections.
The parallel Windows PR HTML test hit its deadline despite reporting a measured
824-pixel document and no surface error; cold-start event-loop timing remains
under investigation rather than being treated as a successful acceptance run.


## Async Windows reader, search and keyboard review — 28 October

Search now highlights bounded positive terms/quoted phrases in loaded sender,
subject and snippet fields and in the reading title/shared projection. Gmail
still performs matching and pagination; no local index, body crawl or download
is added. All untrusted text is escaped before StyledText formatting. Six helper
cases cover injection, operators, Unicode/literal regex, retargeting and bounds;
two native search checks inspect actual rendered rows.

The Windows HTML adapter now starts WebView2 asynchronously on Qt's GUI STA,
without Wry's nested blocking message pump. A measured local create call fell
from 377 ms to 5 ms; this measures caller blocking, not total document loading.
Late initialization is cancelled, closed input buffers are released immediately,
and queued host events carry a native-lifetime epoch. A delete-on-close empty
runtime lease protects initialization from directory cleanup. Private controllers
are mandatory. Sender scripts/host objects/dialogs/permissions/downloads are
disabled, network/file subresources are denied, and only host measurement/wheel
scripts execute. A standalone native component test passed cancellation-before-
completion, latest-document replacement, decoded inline PNG, disabled sender JS,
private-profile/settings checks, blocked loopback-image networking, height
measurement, denied external navigation and native-child/lease release.

Fourteen scene groups pass in this continuation (266 checks after the keyboard
review), plus seven CTest groups and that standalone native component case.
Navigation/composer/motion/HTML were rerun after the final focus changes. A
single-account motion run also passes. The archive assertion now observes the
active delegate's actual spring and excludes recycled rows; it no longer samples
a potentially pooled duplicate at an arbitrary 55-ms instant.

Computer-use inspection confirmed the real row-to-reader layout, inline Reply,
repeated Reply staying compact and closed-editor release. It also caught collapsed
folder controls in the accessibility/focus tree and mouse-only account headers.
Closed groups are now hidden/disabled after their closing spring; account headers
have keyboard/accessible actions. New native checks exercise Tab, Shift+Tab and
Space through two accounts. Small muted text and light-theme blue links have
stronger contrast. Layout and available theme/config choices remain intact.

The async candidate's complete Windows process tree measured private working set
of 123.3 MiB baseline, 136.0 with 1,000 rows, 136.1 scrolled, 238.0 with HTML,
140.9 after reader release, 95.2 backgrounded and 126.3 restored. Settled CPU
samples were 0.00% at counter resolution; private commit remains separately
reported. All seven lifecycle checks pass. The recorded binary hash identifies
the pre-keyboard-polish candidate; this is not Linux PSS, a long live-sync soak or
proof of the 200-MiB HTML target. No `luma-webview-*` runtime directory remained
after the run; the zero-cache fixture retained only its 51-byte settings file and
zero cache files. No local mailbox, index, draft, attachment or photo store was
introduced. The previously rejected unrelated test-folder cleanup stays untouched.

The registered Windows CI package at 2d97bc3 passed all 260 packaged checks with
SDK paths removed. Its archive digest was verified after download. Its provenance
reports dirty source; the next packager records exact changed paths for diagnosis,
so this is not claimed as a reproducible final release. The authorized live test
used the existing test profile/saved vault connection, remotely searched already-
read self-addressed Luma test mail, and opened one conversation/body. First-page
search took 1.094 seconds; no mail actions were issued and no raw mail was saved
as evidence. That pre-existing test profile has a custom 5,000-MiB cache setting;
this check did not change it or establish the normal/default cache acceptance.

On the second branch attempt all Linux native scenes passed and AppImage creation
completed. Portable launch caught a wrapper bug: linuxdeploy's AppRun symlink was
followed during redirection, replacing the binary with the wrapper. Packaging now
unlinks that exact staging entry before creating the wrapper and checks the native
ELF before/after. The corrected portable Linux acceptance remains pending CI.
No release tag, full-parity, complete GTK removal or Google approval is claimed.


The new Windows standalone test's cold factory call exceeded 250 ms on the hosted
runner, although completion remained asynchronous. Its timing is now printed as
evidence, while a controller-before-message-dispatch check enforces the async
contract without a hardware-specific fixed deadline. The native GUI diagnostic
now includes adapter-library loading and runtime setup in its caller measurement.
This is not a claim that Windows SDK bootstrap is always below a frame budget.
The Windows checkout also explicitly disables automatic CRLF conversion before
checkout, so the Windows and MSYS Git tools inspect the same bytes. Dirty-source
path diagnostics remain enabled; clean provenance still requires CI confirmation.
A physical-resolution wordmark experiment was reverted after a navigation run
returned an access violation during exit despite completing its assertions.
The working artwork path stays intact pending isolated Qt lifetime investigation;
the failed experiment is not treated as a successful optimization.


The 3a314df Linux branch job completed successfully, including the corrected
AppImage's native scene suite with developer paths removed. Windows stopped at
the cold timing assumption described above; the semantic conformance test passes
locally after the correction. One later local motion run reported multiple
out-of-order phase failures; its checks now record elapsed time and native
visibility/exposure to distinguish lifecycle interruptions from a product defect.
The successful Linux package does not make this unresolved local run a pass.


Downloaded Linux AppImage 3a314df passed all 263 platform-specific portable scene
checks and its SHA-256 matches the package checksum. It is 176,028,152 bytes;
the installed tree is 495,780,513 logical bytes (531,230,720 allocated), separate
from the mail-cache budget. The authorized live Windows engine read was repeated
in a separate zero-cache profile using only the existing vault identity; remote
search, thread headers and body passed, and only settings.json remained. Original
profile preferences were unchanged. A debugger reproduced the later local access
violation in Qt's Windows UI Automation accessibility path, with QtGui/QtQuick and
UIAutomationCore on the stack. It remains a release blocker under investigation;
Linux portable success is not used to dismiss it.


## Accessible text lifetime correction

Qt Quick 6.12's accessible-item implementation snapshots its QTextDocument pointer
when the interface is created. Replacing an editor's default document could leave
that interface pointing at the freed default document; the debugger caught
QTextDocumentPrivate::addCursor receiving freed memory during Windows UIA GetText.
The new binding invalidates that cached interface before replacement and emits a
structure update afterward. It parents the private document to the editor rather
than its wrapper, preserving lifetime through editor/control teardown. Both the
read-only and rich editors use the same helper. Accessibility stays enabled and
resource loading stays blocked.

A regression deliberately creates the accessible interface before binding. The
original implementation fails both reading and composition cases (exit 2); the
fixed implementation passes both (exit 0). It checks current accessible text,
resource denial and editor-owned document release. Seven CTest groups pass. Native
navigation, motion, composer and long-thread checks pass, followed by a second
motion run; phase order and spring timings are normal again with UIA active.
These checks address the reproduced crash rather than ignoring the failed runs.
See Qt's [cached interface implementation](https://github.com/qt/qtdeclarative/blob/v6.12.0/src/quick/accessible/qaccessiblequickitem.cpp)
and [document replacement](https://github.com/qt/qtdeclarative/blob/v6.12.0/src/quick/items/qquicktextedit.cpp).
Fresh packaged Windows/Linux acceptance of this correction remains required.
