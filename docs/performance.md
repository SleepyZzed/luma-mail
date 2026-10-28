# RAM, CPU and storage acceptance

Goals are below 150 MiB at idle and 200 MiB in ordinary use where feasible,
with idle CPU substantially below 0.4% of one core. These are measurement goals,
not enforced process-kill thresholds. Large media/attachments and graphics
drivers can add memory. Accurate HTML, previews and active composition are kept.

0.7 changes:

- GtkListView recycles inbox widget trees instead of retaining a widget tree
  per loaded conversation. Native tests load 1,000 metadata records, scroll to
  both ends, retain offscreen Ctrl/Shift selection, update a recycled row and
  select all loaded records. The tested GTK build bound 205 row trees.
- Clicked rows carry their model identity directly. Selection uses a bitset;
  sorting and refresh restore selection by account/thread key.
- HTML resize measurements run on allocation events, not a permanent frame
  callback. Live views share weakly held ephemeral engine resources. Closing
  the last view releases their owners. Back/forward caching is disabled.
  Sanitized local message pages use related views to share a renderer process;
  stopped previews have an explicit reload control.
- Inactive readers release their bodies and renderer views after 45 seconds
  hidden. Composers and open attachment previews are protected. Restore reloads
  only the selected conversation, without rebuilding the mailbox list.
- The same background boundary releases inbox widget bindings. Its bounded
  metadata, selection and scroll position stay in memory; returning rebuilds
  only the native list's current bindings, without fetching inbox pages again.
- Immutable MIME bodies and retained attachments share buffers across callbacks
  and draft snapshots. Autosave conflict checks project only the current message
  ID instead of re-fetching the entire draft body.

`packaging/test-performance.sh` profiles the actual release AppImage under a
private D-Bus/X11 session. Its JSON includes the main process and all descendant
WebKit, sandbox and helper processes. It measures launch, a 1,000-row inbox,
scrolling, one/three HTML messages, release and background reclamation. RSS,
proportional resident memory (PSS), private memory and CPU are reported separately.
Unreadable sandbox PSS is explicitly marked incomplete rather than counted as
zero. Shared libraries make summed RSS larger than physical memory consumption.
Software-rendered CI is not a substitute for a real GPU/Hyprland desktop.

## 0.7 measured result

The checked release run 40409671509 profiles the published AppImage, with
complete readable process-tree PSS. Values below are means after five seconds
in each phase; CPU is percentage of one core. Summed RSS counts shared pages
more than once. The fixture has no live accounts/network and uses software
graphics under Xvfb; HTML phases are short and may still include engine startup.

| Phase | PSS MiB | Summed RSS MiB | CPU % |
| --- | ---: | ---: | ---: |
| Baseline | 265.77 | 297.89 | 0.00 |
| 1,000 metadata records | 281.89 | 314.02 | 0.00 |
| Scrolled 1,000 records | 281.93 | 314.05 | 0.00 |
| One HTML view | 523.55 | 716.41 | 9.04 |
| Three related HTML views | 633.50 | 830.28 | 16.67 |
| HTML released | 311.09 | 343.31 | 0.00 |
| Hidden and reclaimed | 308.50 | 340.71 | 0.00 |

Peak HTML PSS was 640.18 MiB. The raw JSON includes unsettled peaks and every
sample. This does **not** meet the requested RAM goals, and the short HTML
CPU intervals do not establish steady idle CPU. Reclamation released the
renderers and all bound inbox widgets, but did not return the entire process
to its initial footprint. Do not describe 0.7 as a sub-150-MiB client based on
these measurements. Compare rendering backends and per-process ownership,
then confirm on an actual GPU desktop before choosing further defaults.

## Cairo comparison on the same binary

[Comparison run 40412013880](https://github.com/SleepyZzed/luma-mail/actions/runs/40412013880)
downloads the published AppImage and verifies its exact checksum, then runs the
same fixture with `GSK_RENDERER=cairo`. It does not rebuild or change Luma.

| Phase | Default renderer PSS MiB | Cairo UI PSS MiB |
| --- | ---: | ---: |
| Fresh baseline | 265.77 | 136.59 |
| 1,000 records | 281.89 | 149.25 |
| One HTML view | 523.55 | 409.63 |
| Three HTML views | 633.50 | 512.00 |
| Hidden after reading/reclamation | 308.50 | 187.68 |

Cairo's summed RSS was 159.37 MiB at baseline and 172.41 MiB with 1,000 rows;
PSS is not the same as the RSS some desktop monitors display. Scrolled-list,
released-reader and reclaimed settled CPU means were 0%; initial baseline
still averaged 2.31% over its short post-settle interval. HTML intervals averaged
9.71%/8.61%. This comparison uses software graphics on different disposable CI
hosts, not a real GPU A/B test or a live mailbox. It demonstrates a useful
renderer cost difference, not a universal memory or animation-performance promise.

To try the optional GTK UI backend on a desktop, quit the existing instance
through its tray menu, then launch:

```sh
GSK_RENDERER=cairo ./Luma-Mail-0.7.0-x86_64.AppImage
```

Omit the variable to return to GTK's automatic renderer. The default remains
unchanged because GPU acceleration can improve animation and scrolling on real
hardware. HTML still uses the full sandboxed WebKit engine; this option does not
make it meet the 200-MiB goal. Further per-process/renderer attribution, real
desktop frame timing and the live soak remain required before changing defaults.

For a three-hour test while the actual Linux app syncs and is used normally:

```sh
python3 packaging/profile-processes.py --attach "$(pgrep -n -x luma-mail)" 10800 live-performance.json
```

Run from the source directory, with exactly the intended Luma instance running.
The profiler reads process counters, not mail, tokens, window titles or arguments.
It never stops or reconfigures the app. The report is an explicitly saved test
artifact, not an automatic app cache. Repeat with Inbox scrolling, large mail,
composition and tray restore. Several-hour real sync/desktop acceptance remains
a 1.0 gate; a short synthetic CI run does not establish it.

Disk categories remain explicit in `docs/storage.md`: installed AppImage/native
libraries, required preferences/secure credentials, optional bounded mail cache,
and temporary graphics/codec metadata. A zero mail-cache budget creates no
mail-body, draft or attachment archive. User-requested saves are separate files.

## 0.8 release measurement

[Run 40425766520](https://github.com/SleepyZzed/luma-mail/actions/runs/40425766520)
profiles the exact 0.8 published AppImage. The complete process-tree PSS means
were 266.33 MiB fresh, 282.53 MiB with 1,000 metadata records, 526.77 MiB with one
HTML view, 635.77 MiB with three, and 309.28 MiB after background reclamation.
Initial and final settled CPU samples averaged 0%; the scrolled list averaged
1.97%, one HTML view 4.66%, and three HTML views 13.53% in their short intervals.
The raw JSON includes startup peaks. This software-rendered CI run still misses
the RAM goals and cannot substitute for a sustained real GPU/mailbox test.

The signature editor adds one short-lived view in the existing ephemeral engine,
only when explicitly opened. Composing does not open a signature renderer or
poll signature settings in the background. Native tests now prove both editor
and WebKit-widget destruction on close, after catching and correcting a window/
owner reference cycle before release. Signature source is limited to 64 KiB and
eight hosted pictures; previews share bounded worker permits. No extra font or
signature database, browser profile or image spool is introduced. The signature
zero-cache report has no files; essential preferences and temporary session/
codec/graphics metadata are reported separately in docs/verification.md.

## 0.9 implementation changes

Closed compact/inline composers explicitly release their owner, GTK tree and
recipient popovers, including when their auxiliary window was never realized.
Focus is cleared and the IM context reset before removing the editor. The native
regression checks assert complete owner/widget release. Windows reader IPC and
geometry now use channel/load/resize/scroll/dialog events, without a permanent
frame callback or 60 ms polling timer. Signature histories share image previews
and bound text snapshots. These are implementation improvements; the earlier
process-tree RAM figures are not relabelled as 0.9 measurements.

## 0.9 release measurements

[Release run 40529376447](https://github.com/SleepyZzed/luma-mail/actions/runs/40529376447)
passes both complete process-tree audits on the exact published build. Linux uses
the default renderer with software graphics, no live network/mailbox and the
same synthetic fixtures as earlier runs:

| Phase | Linux settled mean PSS MiB | Settled mean CPU, one core |
| --- | ---: | ---: |
| Fresh | 266.56 | 0% |
| 1,000 metadata records | 282.87 | 0% |
| Scrolled list | 282.84 | 0.20% |
| One HTML view | 524.96 | 5.22% |
| Three HTML views | 634.46 | 15.74% |
| Reader released | 311.79 | 0% |
| Reclaimed background | 309.25 | 0% |

PSS is complete for every phase. The three-view PSS peak is 643.40 MiB; raw
startup/transition CPU peaks are retained in the published performance.json.
The RAM goals remain unmet. These short settled intervals do not prove zero
CPU during real background Gmail polling or several-hour desktop use.

Windows reports private working set, private commit and summed working set for
Luma plus recursively found renderer/helper children, not Linux PSS. Its hosted
Server last samples are 17.53 MiB fresh, 37.17 MiB with 1,000 records, 104.32 MiB
with one HTML view, 140.23 MiB with three and 38.94 MiB reclaimed. Renderer phases
contain seven/nine processes, returning to one after release. Settled last
samples report 0% CPU; the first fresh sample has no prior CPU delta. These are
last observations rather than phase means, and are not representative guarantees.

The [raw Windows process-tree report](https://github.com/SleepyZzed/luma-mail/releases/download/v0.9.0-dev.1/luma-0.9-windows-performance.json)
is attached to the release (SHA-256
`5fe4381636a3a7f3b1131b793d940203bcbd4ada6787dac2e9a2bd20c3e6d8f4`).
Its recorded executable hash matches luma-mail.exe inside the downloaded Windows
ZIP. The Linux raw samples are published as performance.json alongside it.

On the physical Windows laptop, a local 0.9 candidate's PowerShell 5 audit
measures 92.56 MiB fresh, 104.20 MiB with 1,000 rows, 202.11 MiB for one HTML view,
243.16 MiB for three, and 97.36 MiB reclaimed. The local signature/HTML/audio/tray
checks and relocated package also pass. Fonts, graphics and OS/runtime differences
matter: neither the smaller Server figures nor private working set should be used
as a claim that the Linux or all-use 200-MiB target is solved.

The observer's PowerShell 5 JSON bug is fixed: phase strings no longer carry
file-provider object graphs into serialization. Its complete report is about
12 KiB, rather than an out-of-memory failure. Post-exit zero-memory samples are
excluded. Zero-cache checks create no retained mail/draft/attachment store.


## Fluid asynchronous reader follow-up — 28 October

The local synthetic process-tree run recorded Windows private working set of
123.3 MiB idle, 136.0 with 1,000 rows, 136.1 after scrolling, 238.0 with HTML,
140.9 after release, 95.2 backgrounded and 126.3 restored. All seven lifecycle
checks passed, renderer process counts returned from nine to three, and settled
CPU deltas were 0.00% at sampling resolution. Private commit is separately retained
in validation/fluid-resource-async-windows.json. The artifact records its exact
binary hash and precedes the subsequent small keyboard/contrast changes. It is
not Linux PSS, a real-GPU measurement or several-hour live polling acceptance.
HTML still exceeds the 200-MiB goal.

Direct asynchronous WebView2 initialization reduced measured GUI caller blocking
from 377 ms to 5 ms locally. That comparison measures the constructor call, not
total browser/document load time; no memory trimming or disabled browser sandbox
was used. The Qt event loop remains available during initialization/cancellation.


## 0.10.0-dev.2 resource checkpoint

Menu/editor changes preserve bounded RAM and create no persistent store. Editor
menus are lazy and release after closing. Full local Windows process-tree audit
(source/hash in fluid-resource-popup-windows.json) measures 134.6 MiB private
working set idle / 255.4 with HTML / 64.0 backgrounded. The matching a23a207 Linux
software CI PSS is 226.4 / 402.4 / 326.5; its RAM goals remain unmet. The separate
Windows CI host's 66.1 / 136.4 / 51.6 private-WS result is not a direct comparison
with local graphics. Settled CPU counters read 0.00% at measurement resolution;
real long-duration syncing is still required. See fluid-validation.md for phase
measurement corrections, exact binary/source identity and installation bytes.
