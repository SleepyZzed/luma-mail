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
