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
set to a JSON path. Its 17 native checks exercise the actual form/pipe session,
To-field focus, Tab/Shift-Tab, selected rich formatting, keep/close policies,
quit cancellation, input release, inline reply docking, separated quoted history
and repeated Reply without maximizing. It never writes/sends real Gmail mail.
Transport mock tests separately cover serialized saves, latest input during a
save, one-draft reuse, duplicate-send prevention, conflicts and uncertain writes.
The native document suite covers logical font scaling, paragraph lists, safe
links, Unicode legacy formatting, bounded undo and blocked resource loading.

## Local evidence

27 October 2026, Windows, Qt 6.12/UCRT64: CMake release build and the spring suite
passed. The real native-window motion and composer suites passed on synthetic
data. Core tests: 125 passed and one isolated OS-vault test intentionally ignored;
engine tests: 13 unit tests and three real-pipe tests passed. Strict Clippy is a
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
the real-pipe zero-cache checks verify no compose store or body-cache directory
is written. This does not replace the final installed-app/process-tree audit.
