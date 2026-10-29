# Workspace resume and background rendering

The reported case is Hyprland on a Radeon RX 9070: returning to Luma can stall
the desktop for roughly ten seconds, followed by a black window. That physical
GPU/compositor failure is not reproducible from this Windows development host.
The changes below fix verified application lifecycle gaps; they are not a claim
that a particular kernel/Mesa GPU timeout has been reproduced or eliminated.

## Application changes

- A native window's mapped visibility and its compositor exposure are tracked
  separately. A window on another workspace can remain visible to `QWindow`
  while `isExposed()` is false. One event-driven observer is shared per window;
  no frame polling, workspace IPC or permanent sampling timer is added.
- Main and detached composer windows keep independent exposure state. Springs,
  visible-avatar requests and attachment-thumbnail work can stop when their
  relevant window is no longer presented. A visible composer does not falsely
  mark its hidden main window as exposed.
- Composer springs pause without losing their bounds or momentum during the
  first native-window exposure. The pop-out handoff still expands from the
  original inline card instead of snapping to a full-sized rectangle.
- The Linux HTML adapter previously received a `paused` flag without using it.
  It now hides the browser item and moves an inactive page into WebEngine's
  `Frozen` state. Returning makes the same page active; it is not discarded,
  reloaded or fetched again just because the workspace changed.
- Layout measurement, document scrolling and pending page changes are deferred
  while inactive. Several hidden updates coalesce into the latest page. A late
  completion for an old page cannot make the new revision appear ready.
- The Windows adapter also stops hidden/covered document measurement. Ordinary
  explicit hide/minimise reclamation and closed-reader destruction remain in
  place. Compositor non-exposure alone does not trigger GPU-context teardown.

## Launch compatibility and resource limits

The established Wayland renderer selection is unchanged: detected hardware
Vulkan remains preferred, with the existing OpenGL/software preflight fallbacks
and explicit environment overrides. This patch does not force the OpenGL path
that previously failed to launch on the maintainer's Linux machine. Choosing
Vulkan is not a request for continuous rendering or high GPU usage.

No mail database, disk image cache, browser history or new persistent store is
introduced. HTML profiles stay off the record with `NoCache`; suspended pages
retain only their existing bounded session state in RAM. Closing/collapsing the
message or closing the reader still destroys its view and releases the profile.
Original-body cache settings, remote search, Gmail drafts, authentication,
background synchronization and notifications are unchanged.

## Regression coverage

`workspace-exposure` injects compositor-style exposure events while a window
stays mapped, exercises twenty return cycles, checks independent windows and
checks that idle exposure state emits no repeated notifications.

`private-reader-workspace-lifecycle` runs the real Linux Qt WebEngine adapter:
modal freeze/resume, ten mapped-window exposure cycles, unchanged document-load
count, no hidden frame swaps after settling, deferred resize/hidden updates and
the original off-record/no-cache profile. Private Qt headers are used only by
these test executables to simulate platform events, never by the application.
The render-thread frame counter is atomic, and test services/profiles outlive
the scene and QML engine. CI repeats reader teardown to catch lifetime failures.
The existing component and native/portable UI suites cover minimise/restore,
composer pop-out, HTML layout, mail actions, recovery and bounded cache behavior.

Linux CI uses Xvfb and software graphics. It is build/lifecycle regression
evidence, not RX 9070 GPU utilization, Hyprland acceptance or proof of zero CPU.

The Windows process-tree profiler now validates parent/child creation dates.
Windows retains an orphan's numeric parent ID, which can later be reused by a
Luma helper; following those IDs alone can incorrectly include unrelated apps.
The ancestry tests cover that case, descendants, unordered WMI records and
missing timestamps. This corrects resource evidence, not application RAM use.

## Target-desktop acceptance

Use the new AppImage without adding a new OpenGL override. Check inbox-only,
plain text, an HTML newsletter and a detached composer across repeated workspace
switches, including a longer inactive interval. Verify that the message returns
without a new fetch, that closed message images release, and that notifications
and synchronization still work while another workspace is active.

If a system-wide stall remains, record the corresponding kernel log lines for
`amdgpu`, GPU resets, ring timeouts or page faults, plus the Mesa/Hyprland version.
Those distinguish a GPU-driver reset from a page/layout stall. `QSG_INFO=1` can
print the actual selected Qt renderer for a temporary diagnostic launch. Do not
change compositor synchronization, disable sandboxing or replace working driver
settings as part of this application fix.
