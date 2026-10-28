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
  attachments. Limits: 100 messages, the existing body budget, 32 files and 16 MiB
  attachment bytes. Oversized operations fail with the editor preserved; nothing
  is silently truncated. Newsletter visual styling is not reproduced inside the
  readable forwarded-history section. No history archive is written.
- Previews/isolated acceptance windows register no tray icons. Normal launches
  retain a single per-profile process owner on both platforms.
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
  executable path/app identity. That is OS integration, not mail/cache data.

## Original criteria

| Criteria | Current automated coverage | Remaining acceptance |
| --- | --- | --- |
| 1–5: accounts, scoped/unified folders and identity | Two-account protocol/native navigation, bounded pages and account keys | Two real accounts on target Linux, vault/restart and mailbox comparison |
| 6: conversations | 2,503-header fixture, individual expansion/cancellation, private HTML and recovery | Real MIME/GPU/Wayland/fractional scaling |
| 7: composition/forwarding | Serialized writes, rich editor and forwarding/mode tests | Exact-build live delivery/recipient rendering |
| 8: Gmail drafts | Save/close policy, conflicts and stale callbacks | Linux restart/concurrent clients, complex external HTML |
| 9–12: mail/label actions | Provider/model/account routing and Undo tests | Both real mailboxes with disposable fixtures only |
| 13: remote search | Scoped search/highlighting/pagination | Real old/giant mailbox traffic |
| 14: attachments | RAM-only previews/media/exports, clipboard and MIME | Inline-image authoring, actual codecs/portals |
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
