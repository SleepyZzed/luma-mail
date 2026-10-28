# 0.11 implementation and 1.0 acceptance audit

Implementation, automated fixtures and real-account acceptance are distinct.
Simulated accounts are never recorded as live Linux acceptance.

## Current batch

- Send is separate from Close/draft policy, including reader reconciliation,
  queued/cancelled sends and terminal sent-session cleanup.
- Reply/forward composers offer Reply, Reply all, Forward single and Forward all.
  New messages and saved drafts do not offer mode switching. Authored input and
  attachments survive; source account/conversation and revisions are checked.
- Forward all loads readable history chronologically on demand and deduplicates
  attachments. Limits: 100 messages, the existing 128 KiB quote budget, 32 files and 16 MiB
  attachment bytes. Oversized operations fail with the editor preserved; nothing
  is silently truncated. Newsletter visual styling is not reproduced inside the
  readable forwarded-history section. No history archive is written.
- Previews/isolated acceptance windows register no tray icons. Normal launches
  retain a single per-profile process owner on both platforms. Local activation
  keeps its acknowledgement socket alive until the client reads/closes it,
  preventing intermittent final-byte loss on Windows named pipes. Repeated
  activation and stale-owner recovery have dedicated component coverage.
- Linux daemon notifications have explicit Open actions. Windows uses native
  actionable toasts with individual targets. Unambiguous 4–8 digit codes in the
  existing subject/snippet metadata offer Copy code; no extra body fetch occurs.
  Privacy mode suppresses codes. At most 64 in-memory targets are retained and
  code actions expire after ten minutes. Settings has a synthetic notification
  test. OS notification settings/DND remain under the user's control.
- External image autoload is opt-in; attachment thumbnails have a separate
  preference. Existing byte/pixel/concurrency limits apply. Collapsing a message
  releases rendered images; received thumbnails disappear with their last visible
  card. Switching conversations closes received attachment previews. No image
  disk cache is added; the existing bounded original-body cache is unchanged.
- Windows toasts require a per-user Start Menu shortcut holding only the
  executable path/app identity and a per-user COM activation registration. Action
  arguments contain random expiring identifiers, never codes or mail content.
  These are OS integration, not mail/cache data.
- Insert picture in body retains at most eight images within the existing
  16 MiB attachment RAM budget (8 MiB per input). Only small decoded thumbnails
  cross into the editor; Gmail draft MIME holds the original bytes. Send waits
  for insertion acknowledgement; cancellation cancels a queued Send. HTML uses
  multipart/related for CID images, multipart/alternative for plain/HTML bodies,
  and multipart/mixed for ordinary attachments. An ordinary attached picture may
  appear below quoted history in other clients; insert it in the body to control
  its authored position. Recipient clients still control wrapping and dark mode.
- Wayland startup prefers a detected hardware Vulkan device with graphics and
  swapchain support, with OpenGL/software preflight fallbacks. Explicit renderer
  environment overrides remain supported. This is not a target-driver runtime
  guarantee. Text morphs use transforms; heavy message-body layout starts after
  the projection. The AppImage requests the installed desktop chooser portal;
  a Qt fallback uses Luma's palette. Native portal availability is a host setting.

## Original criteria

| Criteria | Current automated coverage | Remaining acceptance |
| --- | --- | --- |
| 1–5: accounts, scoped/unified folders and identity | Two-account protocol/native navigation, bounded pages and account keys | Two real accounts on target Linux, vault/restart and mailbox comparison |
| 6: conversations | 2,503-header fixture, individual expansion/cancellation, private HTML and recovery | Real MIME/GPU/Wayland/fractional scaling |
| 7: composition/forwarding | Serialized writes, rich editor and forwarding/mode tests | Exact-build live delivery/recipient rendering |
| 8: Gmail drafts | Save/close policy, conflicts and stale callbacks | Linux restart/concurrent clients, complex external HTML |
| 9–12: mail/label actions | Provider/model/account routing and Undo tests | Both real mailboxes with disposable fixtures only |
| 13: remote search | Scoped search/highlighting/pagination | Real old/giant mailbox traffic |
| 14: attachments | RAM-only previews/media/exports, clipboard, inline-picture insertion and MIME | Real recipient rendering, codecs/portals |
| 15: Drive | Upload/progress/cancel/recipient grants | Interruption/sharing recovery, existing-file selection and recipient access |
| 16: notifications/tray | Single owner, daemon transport/actions, Windows adapter and toast XML | Actual OS appearance/clicks, DND/suspend/bursts |
| 17: Noctalia | Config validation, palette monitoring and theme tests | Installed Noctalia/Hyprland/wallpaper changes |
| 18–19: cache/performance | Zero-cache/TTL/LRU/allocation, 1,000-row virtualization and process-tree profiles | Sustained real usage, GPU and unmet Linux RAM goals |
| 20: quality/completeness | Component/portable UI groups, recovery and keyboard improvements | Authoring/Drive work and full accessibility/security/privacy review |

Google approval is unconfirmed. This Windows workspace has no WSL/Linux
installation or remote connection to the user's Linux desktop. Linux CI verifies
a Linux build and synthetic desktop/accounts; it cannot certify real Gmail,
keyring, GPU, Noctalia or notification behavior on that desktop. These remain
1.0 gates. Exact live-account and build evidence is recorded separately.

## Local Windows evidence before the activation fix

[The two-account read check](validation/fluid-live-two-read-0426658.json) uses
registered source 0426658 and the user's two connected accounts. Both scoped
inboxes return bounded 25-row pages in 1.61/1.13 seconds, with zero cache files
and no mailbox writes. It does not certify sending or Linux account behavior.

[The full-process synthetic profile](validation/fluid-local-resources-9f780b9.json)
uses local source 9f780b9, a debug engine and Qt 6.12.0 on Windows. Private working
set is 139.3 MiB at baseline, 153.3 after scrolling 1,000 rows, 253.5 with HTML,
155.5 after releasing HTML, and 64.1 after background reclamation. Settled sampled
CPU is 0.000% of one core. This short fixture is not a sustained live-mail test;
the HTML phase exceeds the requested 200 MiB target. These counters are not Linux
PSS, and working-set sums count shared pages differently. No zero-budget mail
cache files were written. Package CI resource reports have their own build IDs.


## Published source and live transport acceptance

[v0.11.0-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.11.0-dev.1) is built from `d04f531b748ab707a5751f9391dba3f1cbc4654e`. Eight component groups per
platform, 305 Linux/308 Windows portable assertions and the core/protocol/Clippy
gates pass. [The live Windows run](validation/fluid-live-two-0.11.0-dev.1.json)
passes 29 checks on two authorized Gmail accounts, including saved draft removal,
recipient CID/attachment delivery, reply unread state, forwarding and mailbox
actions. Fixtures were moved to Trash; zero cache files were written. Actual
Linux and phone UI, OS notification actions, sustained resources and Google review
remain distinct. Canonical CI resource reports now describe this source; the
separate earlier local debug and read-only results retain their own identities.
