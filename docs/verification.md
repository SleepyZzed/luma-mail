# Verification log

## 0.7.1 composer hotfix and 0.8 signature validation

[Hotfix release run 40419941734](https://github.com/SleepyZzed/luma-mail/actions/runs/40419941734)
passed all Linux/AppImage gates and published v0.7.1-dev.1 on exact head
`5be7834b87ba33299c6fa8b587cee0820d9c53f7`. The downloaded 157,858,296-byte
AppImage matches SHA256SUMS and GitHub's asset digest:
`3aabda2f3f278150b45660254634a406ceaa1f34a31efb894832942e5d353e1e`.
This restores the individually wrapped, visibly bounded toolbar, tests real
Bold/Bullet/Numbered button actions and verifies Ask/Cancel/Close, duplicate-close
protection, Manual and incomplete conditional Save-on-close without remote/local
draft creation. A GTK4.12 width issue caught by CI was corrected before publishing.

The 0.8 work passed Windows strict Clippy, 117 core and eight desktop tests,
plus native compose, recipient/navigation/font/history/conflict, draft-policy,
selection and 1,000-record virtual-list diagnostics. Core signature tests cover
projected reads, signature-only PATCH, remote-change refusal, safe rich HTML,
12 save/reopen cycles, separate MIME body/signature/quote and merge recognition.
Mocked Drive tests cover optimized public-image upload/read-only link permission,
invalid/cancelled sources and refusing an untrusted upload destination.

The authorized real account passed `signature-read` and
`signature-mail --allow-self-mail`: three draft reopen/save cycles, one delivered
rich signature/table/link/public project-logo URL, removal of the sent Gmail
draft and an unchanged actual account signature. The synthetic message was moved
to Trash. No real signature setting was written and no public Drive copy was
uploaded. Public-image permission writes remain mocked, not live acceptance.

The actual Linux/WebKit formatting check caught a transient script-world bug;
the editor now registers its isolated world for the view lifetime. Formatting,
font/size/further typing, links, bullets, tables, public picture serialization and
separate draft restoration subsequently passed. The close-cancel diagnostic
uses the actual response buttons and waits for libadwaita dismissal; emitting a
response signal alone does not reproduce the real button/close lifecycle.
A real retention bug was caught before publication: the window's dispose signal
could not release an editor that itself owned the window, and a second Close
could target a still-closing dialog. Approved close now destroys the window and
releases its owner on native unrealize. A Windows native test passed actual
Keep editing/discard buttons and complete editor/widget release; the Linux gate
also checks destruction of the actual WebKit widget. Signature toolbar height,
real image decoding and continued typing with in-memory picture previews are
checked explicitly. Final storage/package gates are recorded below when completed.

## Final 0.7 release, 26 October 2026

[Release run 40409671509](https://github.com/SleepyZzed/luma-mail/actions/runs/40409671509)
passed on `eec4aa1e5d8136e8ae89f5fe5dacc1876e103dd2` before publishing
[v0.7.0-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.7.0-dev.1).
The downloaded 157,866,488-byte AppImage matches SHA256SUMS and GitHub's digest:
`ae58d47ed2f9f4806996d6543eaf18eff2120d6516aa0ae2916af05dd5161b1c`.
PKGBUILD pins functional source `d4505a15e2276d0f7485b59509828be22db31179`;
its downloaded archive matches
`6f9fc7fc43cf0a34300a5453e20d749b7690d817228126498bbed0d0ffcc0266`.
At the archive check, all 174 files other than the intentionally updated pin
recipe matched the prepared functional source. Later commits update test cleanup
and documentation without changing the published application binary.

Strict fmt/Clippy, 108 core and eight desktop unit tests passed. The opt-in
desktop acceptance ran separately against real GTK/D-Bus, exercising tray and
system-notification transport. Native diagnostics covered compose traversal,
formatting, identity routing, field-level merge choices and memory release,
inline close/repeated Reply, selection, navigation and 1,000 metadata records
with 205 recycled widget bindings. Offscreen selection/actions and suspension
to zero bindings passed. Three sandbox-launcher tests passed.

Native/AppImage HTML and RAM-backed audio/video checks passed on X11 and
headless Wayland, including hidden host WebKit and a read-only installation.
Native reader/compact composer/inline reply screenshots were visually inspected.
The released reader screenshot matches the inspected SHA-256
`5535ace93843ff0be3019a64e9bd9a4c7148ca43bc2a21a82136689b97b25176`.
An earlier Wayland run rendered/played successfully but failed in a document
portal unmount race during fixture cleanup; the harness now rechecks the mount.
No renderer sandbox was disabled to obtain a passing release.

| Disk category | Logical bytes | Allocated bytes on CI |
| --- | ---: | ---: |
| Compressed AppImage | 157,866,488 | 157,868,032 |
| Extracted installation | 497,466,860 | 511,483,904 |
| X11 fixture temporary runtime | 2,729,517 | 3,510,272 |
| App body cache, local drafts and attachment spool | 0 | 0 |
| Other persistent XDG cache files | 0 | 0 file bytes |

The extracted tree is an alternative representation, not an additional mail
archive during normal FUSE operation. Extraction fallback does require its
temporary installation space. Empty directories still occupy blocks. The
Wayland/read-only runtime contained no files; synthetic configuration was 36
bytes. Graphics/codec inventories are separate temporary session metadata, not
body-cache entries. Essential preferences/keyring storage and explicit saves
remain separate from the configured mail-cache budget.

The release includes `performance.json`, measuring the entire main-process
descendant tree. Under software-rendered X11, settled baseline was 265.77 MiB
PSS (297.89 MiB summed RSS); 1,000 rows used 281.89 MiB PSS; one and three HTML
views averaged 523.55 and 633.50 MiB PSS. After hidden reclamation, zero widget
bindings remained and PSS was 308.50 MiB, with 0% sampled settled CPU. The HTML
windows averaged 9.04%/16.67% CPU after a five-second settling allowance; these
short software-rendered intervals do not establish long-term idle CPU.
**The requested RAM targets are not met by this run.** See performance.md.

The same checksum-verified binary also passed a Cairo UI comparison in
[run 40412013880](https://github.com/SleepyZzed/luma-mail/actions/runs/40412013880):
fresh baseline 136.59 MiB PSS, 1,000 records 149.25 MiB, one HTML view 409.63 MiB,
three views 512.00 MiB, hidden after reading 187.68 MiB. This is an optional launch
choice, not a changed default or evidence that all idle/use goals are met.
The corrected Wayland cleanup harness passed the complete check in
[run 40410693307](https://github.com/SleepyZzed/luma-mail/actions/runs/40410693307).
[PR #8](https://github.com/SleepyZzed/luma-mail/pull/8) merged as
`54db04921c0de69f8f529d1688e7e8fbeafdcaec`; its only post-release source change
is the test-fixture cleanup race fix, not an application-binary change.

A fresh authorized real-Gmail check verified the two available Send-as choices,
stale-draft conflict refusal, independent merged copy, preservation of the
original, projected revision checks and draft-only deletion. All synthetic
drafts were removed; this test sent no mail and changed no unrelated mail.
Live alias delivery and two-account Linux acceptance remain outstanding.

Google branding/data access remains under review; no scope was added. This
release remains early access. Target GPU/Hyprland, Noctalia/keyring, sustained
live-sync profiling, the remaining roadmap and security acceptance still gate 1.0.

## Final 0.6 release

[PR check 40379155798](https://github.com/SleepyZzed/luma-mail/actions/runs/40379155798)
and [release run 40379375832](https://github.com/SleepyZzed/luma-mail/actions/runs/40379375832)
passed on `bf7ebe0a0bf86cddef451f41406a4bf6c3ffd15e`. The release includes
source through `642ddfe8c9dad4ed885048e2961526ac0b3af77a`, pinned with its actual
archive checksum in PKGBUILD. [PR #7](https://github.com/SleepyZzed/luma-mail/pull/7)
merged as `1293a122445229eae2e2dcc8dd106abf9f2adacb`.

[v0.6.0-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.6.0-dev.1)
contains the 157,620,728-byte x86_64 AppImage. The actual download matches both
SHA256SUMS and GitHub's asset digest:
`dd480c90ccc2174966508d71fe792245228258fa9005c4dbc4ac34a156450783`.

The gates include strict Clippy/tests, real GTK/D-Bus tray and notification
transport, native composer/clipboard/navigation checks, release compilation,
native and bundled HTML/media on X11 and Wayland, bundled cursor preferences,
isolated-host-WebKit execution and a read-only AppImage. An earlier build was
blocked by a diagnostic call accidentally included in normal sending; it was
removed before the final passing release. No failed build was published.

| Measured category | Logical bytes | Allocated bytes on CI |
| --- | ---: | ---: |
| Compressed AppImage | 157,620,728 | 157,622,272 |
| Extracted installation | 496,950,245 | 510,967,808 |
| X11 rendering fixture's temporary runtime | 2,701,192 | 3,395,584 |
| X11/Wayland fixture app-body cache files | 0 | 0 |
| Other persistent XDG cache files | 0 | 0 file bytes |

The extracted tree is an alternative installation representation, not extra
mail retention during normal FUSE execution. Empty runtime/cache directories
still occupy filesystem blocks. The Wayland/read-only fixture created zero
runtime files; configuration held a 36-byte synthetic settings file. These are
isolated fixture measurements, not a universal runtime-memory/storage ceiling.
The released JSON reports preserve the breakdown, and the existing body-cache
allocation/zero-mode tests pass. See storage.md for the cache/retention limits.

A second authorized Windows test used the real app's Compose and inline Reply
buttons, real Gmail autosave and Send. Each returned Gmail confirmation and
closed its editor. After idle, an exact synthetic-subject API check found two
sent messages (one original plus one reply) and zero matching drafts. Both were
moved to Trash. The actual account photo appeared in the sidebar, list and
reader. The native file chooser could not be operated reliably by the automation;
attachment sequencing is covered separately by the live three-save test below
and isolated Linux clipboard diagnostics, not claimed as a native Windows picker
acceptance result. No user clipboard was overwritten.

The final Windows review tree still contains 2,768,509 logical bytes with zero
body-cache/local-draft files. The UI run did not grow it. This Windows renderer
metadata is separate from Linux mail retention and the installed bundle.
Google's console still reports branding and data access under review. No new
scope was added; target Linux/two-account and other roadmap gates remain open.


## 0.6 local and live checks, 26 October 2026

Strict Clippy and 106 executed workspace tests pass on Windows (one opt-in vault
test remains ignored). Native GTK checks exercise compact/full/Back lifecycle,
To focus, field traversal, rich undo/redo, recipient chips, numbered lists,
read-only reply history, sent-state guards, bulk selection and fixed navigation.

Using the existing authorized account and the same four permissions, fresh
consent/refresh and a real 64×64 own-account Google photo fetch passed. A live
self-mail regression created one draft, updated it three times with attachments,
sent once, verified three attachments and one delivered message, and confirmed
the consumed draft was absent. The synthetic message was moved to Trash. Gmail
search indexing lagged direct retrieval, so verification resumed without sending
again. No unrelated mail was modified.

The Windows review tree remains 2,768,509 logical bytes: 1,621 config plus
2,766,888 renderer/runtime bytes. The synthetic compose test has 310 config bytes
only, no body cache or local drafts. Windows allocation is an estimate; Linux
package/runtime allocation must be measured by the final build. The rich history
cap is covered by a 120-state over-budget regression; history stays in RAM.

Linux/AppImage publication evidence follows after the final source gates pass.
Google branding and data access remain under review, not approved.

## Final 0.5 release

[Release run 40363651714](https://github.com/SleepyZzed/luma-mail/actions/runs/40363651714)
rebuilt `cedbedf3bfdad26ab7c8278d4d15b8b6501e920c` and repeated all Linux gates
before [v0.5.0-dev.2](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.5.0-dev.2).
Its AppImage is 157,563,384 bytes; published SHA-256:
`8089d1affd5fd64eef45fb418359380e352fbfa574f10650e3f2156f28598d78`.
The downloaded AppImage matched SHA256SUMS and GitHub’s asset digest.
The release includes package/runtime allocation JSON reports and synthetic
renderer screenshots. Dev.2 includes the Mesa runtime hardening; dev.1 predates it.
The README/homepage point to dev.2. Google branding/data-access review remains
pending; no additional scope was added during this pass.

## 0.5 hardening, 26 October 2026

Windows strict Clippy and 95 core/5 desktop tests passed (one opt-in OS-vault
test excluded). Native GTK diagnostics pass compact-first compose, To focus,
Tab/Shift+Tab, font/link preservation, full/compact/Back transitions, no local
draft creation, bulk selection, pinned navigation at 100/150%, and persistent
account/unified expansion/collapse with long names. A native visual check
confirmed the compact overlay with initial To focus and unchanged theme/layout.

[Linux run 40361360863](https://github.com/SleepyZzed/luma-mail/actions/runs/40361360863)
passed strict fmt/Clippy, 95 core/5 desktop tests and the separate GTK/D-Bus test,
actual clipboard-image paste and composer transitions, animated navigation,
native/bundled X11/Wayland HTML/media, isolated host WebKit and read-only package
checks. The initial Linux lifecycle test's old full-page assumption was updated
to exercise compact start followed by expansion; no lifecycle gate was removed.

The first audit measured a 157,563,384-byte AppImage and 496,793,202 logical /
510,812,160 allocated bytes extracted. Zero-cache renderer checks created no app
body-cache files or new local drafts/attachment spool. About 2.1 MiB of other XDG
cache allocation prompted additional Mesa runtime/size hardening. Final shader-hardened PR results follow below.

[Final PR run 40363407935](https://github.com/SleepyZzed/luma-mail/actions/runs/40363407935)
repeated all gates successfully on `cedbedf3bfdad26ab7c8278d4d15b8b6501e920c`,
including private Mesa cache location/size configuration. The resulting AppImage
was 157,563,384 bytes; its extracted tree was 496,789,106 logical / 510,808,064
allocated bytes. Native/bundled/isolated X11 reports recorded zero app-cache files
and zero other-XDG-cache files. Private session data (including Mesa, codecs and
test portals) occupied 3,416,064–3,440,640 allocated bytes. The 36-byte fixture
configuration occupied 8 KiB including its directory. These are synthetic test
figures, not a claim that essential user settings/keyring need no storage.
Wayland's fixture wrapper tears down its private runtime before the outer report;
its zero post-run runtime count must not be read as zero live session memory or
an all-driver cache guarantee. Test reports measure allocation, not RAM usage.
The patch was merged through [PR #6](https://github.com/SleepyZzed/luma-mail/pull/6).

A real read-only named-address Contacts query completed without an API error;
the deleted disposable self-contact no longer has a matching photo. A separate
people/me experiment returned 403 with the granted Contacts scope and was removed
from production code. No extra scopes or third-party sender lookups were added.
The exact-address and named-From normalization/transport guards pass tests.

A temporary Windows debug cache stress driver wrote 2,000 × 64 KiB synthetic
bodies through a 1 MiB allocation-aware budget. Usage after both 1,000 and 2,000
writes was 1,040,384 bytes (14 entries); this was also the measured peak. Setting
zero removed the body directory. Writes plus allocation checks took 6,932 ms;
this measures cache I/O, not Gmail latency or a real large-account soak. The
fixture directory and temporary driver were removed. The metadata pager's
100,000-message fake mailbox still fetches only one 25-row page initially.

## 0.4 acceptance, 26 October 2026

The final gated [release run 40347048702](https://github.com/SleepyZzed/luma-mail/actions/runs/40347048702)
repeated all checks on `1c6772c18aa5dbd930b74dc27145497ed0d1a450` and published
[v0.4.0-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.4.0-dev.1).
The downloaded 157,538,808-byte x86_64 AppImage matched SHA256SUMS and GitHub's
asset digest: `8101032b701ad87bb6204cabf4f662a20046c105a2a76fcfe907685c4fa7932c`.

[Run 40344826916](https://github.com/SleepyZzed/luma-mail/actions/runs/40344826916)
passed formatting, strict Clippy, 93 core/5 desktop tests, the separate GTK/D-Bus
tray/notification test, real clipboard-image attachment diagnostics, animated
composer host transitions/cleanup, and account-switch/sidebar geometry checks.
Native and bundled X11/Wayland HTML/media, hidden host WebKit and read-only
AppImage checks passed, including zero-cache operation. This proves those runner
checks, not acceptance on the user's installed Hyprland/Noctalia system.

An isolated Windows review session used the real production Desktop client,
PKCE/loopback callback and OS vault. Only a fixed synthetic subject search was
shown. Incremental read-only Contacts consent completed; a disposable self-contact
with Luma's icon produced the real 96×96 photo in list/reader, then that contact
was moved to Google Contacts Bin. No unrelated contacts were edited.
Gmail draft/save/reopen/send removed the sent draft and delivered the self-mail.
A 17 MiB synthetic attachment uploaded to Drive and delivered as one filename
link to the same account. A random example.invalid Gmail Trash filter was
created, listed and removed with ownership checks. Other-recipient Drive access,
cancellation and sustained/two-account Linux acceptance remain open.

Google People API is enabled, with the optional Contacts scope and usage
justification declared for review. On 26 October 2026 the maintainer-authorized
English narrated demo was uploaded unlisted after a full decode/privacy check.
It includes the real native-browser address bar/client ID and complete four-scope
view, original incremental consent, synthetic Gmail/Drive/photo actions, and a
real Unblock of a disposable rule created through Luma's provider. Captures are
explicitly labelled as an edited still-screen walkthrough; only the account
avatar is redacted and blank vertical space is omitted in two browser views.
Its private review link is held in the submission rather than published here.

The final questionnaire was submitted with an ownership appeal: Search Console
reports the exact homepage URL-prefix as verified, while the automated branding
check rejected ownership. Google Auth Platform now reports: "Your branding and
data access are currently under review." Client-only data flow was documented,
and Google was asked to confirm security-assessment applicability before an
assessment is arranged. No assessment was purchased. Submission does not remove
the unverified warning or user cap; Google's decision remains outstanding.

All four synthetic review conversations were moved to recoverable Gmail Trash;
the 17 MiB upload and disposable contact were moved to their services' Bin. The
synthetic filter was removed by the app's real Unblock action. Unrelated mail,
contacts and filters were not modified.

Development date: 2026-10-25.

Host: Windows; portable MSYS2 UCRT64 toolchain installed inside the task's
`work/toolchain` directory. Rust 1.99.1, GTK 4.24.1, libadwaita 1.10.0;
gtk4 Rust bindings 0.11.5, libadwaita bindings 0.9.2, Reqwest 0.13.5.

Passed on the development host:

- `cargo fmt --all --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace` (50 core tests and one tray menu test)
- `cargo build --locked --workspace`
- `cargo build --locked --release --workspace`
- Native GTK launch and automatic exit with `--smoke-test`.

The preview PNG is rendered from the running GTK window through GTK/GSK, using
an isolated test configuration/cache directory. No account fixture is injected.

Linux CI: [run 40206680811](https://github.com/SleepyZzed/luma-mail/actions/runs/40206680811)
on Ubuntu 24.04, source `b27c6ce9cd49aa5c8239bc2dfc7a56bd246200df`.
Formatting, strict Clippy, 51 regular tests and the separate desktop D-Bus
acceptance check passed. The latter exercises GTK close-to-tray, restore and
menu Quit against an isolated
StatusNotifier watcher, and verifies that a labelled test notification reaches
the standard `org.freedesktop.Notifications.Notify` D-Bus endpoint while hidden.
That fixture tests transport; it does not prove how a particular desktop daemon
displays notifications or simulate new Gmail messages.

[Run 40206359687](https://github.com/SleepyZzed/luma-mail/actions/runs/40206359687)
also passed the optimized Linux build, native GTK launch, AppImage packaging,
and launch of the bundled AppImage with extraction mode on Ubuntu 24.04. Run
40206680811 also passed all those checks with the additional D-Bus acceptance
test. A subsequent release attempt exposed a restored staging-directory
collision; packaging now uses an automatically cleaned private temporary path.

Final release verification: [run 40207648619](https://github.com/SleepyZzed/luma-mail/actions/runs/40207648619)
passed every build/test/package/launch step and published
[v0.1.0-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.1.0-dev.1)
from source `5f2929cd649e989b7872dc0e7dcc60342ecc9053`. The downloaded
41,961,976-byte x86_64 AppImage matched both the published SHA256SUMS and GitHub's
asset digest: `a1e7b790c079f1f32653df24204f292e469f58f13daf88c3ce9b603a6f33fc7f`.

Not executed: real Google OAuth consent or Gmail/Drive
operations, actual Secret Service D-Bus interaction, Noctalia/Hyprland/Wayland
integration, external notification display, makepkg/AUR publication.

See `testing.md` for the manual test procedure and `STATUS.md` for outstanding
features. A passing local build is not a substitute for those acceptance checks.

## 0.1.1 development hardening

The first Linux report found a missing-OAuth setup loop and incorrect system
cursor behaviour in the AppImage. The account issue was confirmed to occur
before any Desktop client JSON had been imported. No real Gmail consent was
completed during that report.

[Run 40219055786](https://github.com/SleepyZzed/luma-mail/actions/runs/40219055786)
passed the guided-window regression check and the new bundled cursor settings
check, including dconf transport and session overrides. Actual cursor artwork
and fractional scaling on the reporter's compositor remain to be retested.

The 0.1.1 host build passes formatting, strict Clippy, 56 core tests plus one tray
test, an optimized build and a native launch of the guided sign-in empty state.
New regressions cover burst notification continuation, body-lifetime network
limits, backoff fairness, OAuth-client validation, cross-account label names and
search-date/query validation.

[Run 40220277862](https://github.com/SleepyZzed/luma-mail/actions/runs/40220277862)
checks the complete 0.1.1 implementation, including GTK selection/page retention
and persistent label navigation. The acceptance fixture uses synthetic rows only
inside the test executable; the shipped application has no fake inbox data.

Final 0.1.1 release: [run 40220658468](https://github.com/SleepyZzed/luma-mail/actions/runs/40220658468)
passed every check and published
[v0.1.1-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.1.1-dev.1)
from source `221c7a8c6619221ea963be0f16ca185c310b0710`. The downloaded
42,093,048-byte AppImage matched SHA256SUMS and GitHub's asset digest:
`10204ded4f2a672e1ea1446adb92c72cf06b18f59f94b49a81566c452d867a72`.
The native package recipe is pinned to the same Rust implementation in
`dd6db7d67d0c999a09f0d4e6920a290bad0afde7`; the release commit adds only native
package/checklist/documentation updates. The recipe's codeload SHA-256 was also
checked. makepkg/AUR and real Google/Noctalia acceptance remain unexecuted here.

## 0.1.2 button-only sign-in

Source `466c707fbcf49ba2835d1b666e779d481afaef6a` removes both JSON import
screens, the file loader and runtime client override. The maintainer supplies
Google's public Desktop client registration at build time. The new keyring
client-ID binding rejects legacy or different-client refresh tokens before any
network request, and reconnecting requires a grant for the current app identity.

Local verification passed formatting, strict Clippy, 57 core tests, two desktop
unit tests and a native GTK launch of the unconfigured sign-in state. A separate
check confirmed that `require-google-client` rejects an unconfigured build and
accepts a synthetic registration fixture in the test executable. The fixture is
not an application login, user credential or published binary configuration.
The native recipe's pinned source checksum and both shell scripts' syntax were
also checked. Native makepkg remains unexecuted.

Linux verification: [run 40223256605](https://github.com/SleepyZzed/luma-mail/actions/runs/40223256605)
passed formatting, strict Clippy, all unit tests, the real GTK/D-Bus acceptance
check (including the removed-import UI assertions), native launch, AppImage
build/launch and cursor settings transport. This run preceded Google provisioning
and tested an unconfigured developer bundle.

Luma's Google Desktop client and both repository build values were then created.
Google's initial audience was External Testing with the chosen test account listed.

Configured release [run 40224992927](https://github.com/SleepyZzed/luma-mail/actions/runs/40224992927)
passed every build, test, launch, package and publish check, including the required
app-registration feature and validation of the real build-time values. It published
[v0.1.2-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.1.2-dev.1)
from `35cd52c500329c9884802715c15e2978cf6b78f0`. The downloaded 42,097,144-byte
AppImage matched both SHA256SUMS and GitHub's digest:
`59423a3c26feea44545061270e71bb4e614f96a5756f139a301c669dd6a438eb`.
The exact-tag source ZIP passed its archive integrity check.

Live Google consent/keyring tests and broad public Google verification remain
unexecuted. The 0.1.1 binary does not acquire these changes automatically;
install the configured 0.1.2 AppImage for the next real Linux acceptance pass.

## Public early-access audience — 25 October 2026

- Published repository-backed homepage, privacy policy and terms at
  [sleepyzzed.github.io/luma-mail](https://sleepyzzed.github.io/luma-mail/).
  [Pages deployment 40226549143](https://github.com/SleepyZzed/luma-mail/actions/runs/40226549143)
  succeeded. Homepage, privacy, terms and CSS all returned HTTP 200; the native
  browser rendered the homepage successfully.
- Saved those URLs and `sleepyzzed.github.io` as the authorized branding domain
  in the existing Google project. No OAuth scopes or user grants were expanded.
- Selected Publish app and confirmed publication. Google Auth Platform visibly
  showed **In production**, **External**, verification required, and **1 / 100
  user cap**. The test-user list is no longer applicable. This is configuration
  evidence, not a completed real-account consent or keyring test.
- Kept the existing release tag, source and AppImage bytes unchanged. The audience
  change applies to the registration already compiled into the checked 0.1.2
  AppImage. The release remains a prerelease and now describes early access.
- Assessed Gmail IMAP/SMTP OAuth, app passwords and JMAP against the original
  criteria. None removes Google's consent requirements while preserving the
  requested button-only Gmail experience. No provider migration was performed.
- A browser-only request using Luma's registered client ID, Gmail modify scope,
  S256 PKCE and a loopback return address reached Google's real account chooser,
  displaying Luma Mail and the published privacy/terms links. No account was
  selected and no permission, callback or token exchange was completed. This
  checks the OAuth entry point, not an installed-app end-to-end login.
- [Linux run 40226522760](https://github.com/SleepyZzed/luma-mail/actions/runs/40226522760)
  passed the full build/test/AppImage workflow for homepage source commit
  `328af0ce7f5d5154ccf0d7cb63c2dbf41e471a0a`. Rust application sources did not
  change in this publication batch. GitHub's release API confirmed prerelease
  status and the unchanged AppImage digest after the release-note update.

The current protocol decision and verification limits are in [oauth.md](oauth.md).
Formal verification, uncapped sign-in and the 20 live acceptance checks remain
separate full-release requirements.

## 0.2.0 mail experience and authorized live checks

Historical "not executed" notes above describe earlier releases. Subsequent
authorized testing completed browser consent/PKCE/loopback, Windows Credential
Manager, token refresh and synthetic self-mail draft create/reload/edit/conflict,
send, inline photo/DOCX/audio attachments, search/star/read/labels, archive/Inbox
and Trash/restore. Gmail archive responses were checked for labels without INBOX.
Empty projected attachment-search and bodyless Trash POST handling were fixed
from observed responses. Private mailbox content is not in the repository.

A bounded metadata test measured Inbox 25/25 snippets 0.93s, next page 0.70s,
Archive 1.12s and All Mail 0.88s; network/account/quota conditions affect results.
With explicit permission approval, a random example.invalid sender filter was
created, read back and ownership-checked deleted; its absence was confirmed.
Google normalizes away redundant remove-INBOX on a TRASH filter, now tested.
No existing mail was changed by this filter test.

[Linux run 40235667937](https://github.com/SleepyZzed/luma-mail/actions/runs/40235667937)
passed tests, GTK/D-Bus acceptance, release build, AppImage packaging/launch and
cursor settings. It did not instantiate WebKit. Deeper rendering run 40237135136
exposed Ubuntu 24.04's namespace/AppArmor restriction. Luma now probes and falls
back to text; CI grants userns specifically to its disposable runner bwrap.
WebKit's sandbox and the global OS restriction remain enabled. Final rendering,
media and 0.2.0 release results will be recorded after completion.

Remaining: second account, target Secret Service/Wayland/Noctalia, real incoming
notification presentation/clicks, Drive recipient access, sustained giant-mailbox
profiling, Google verification and the full control audit.

## Composer and newsletter regressions — 25 October 2026

Native Windows GTK diagnostics passed To focus, recent-recipient completion,
Tab/Shift+Tab through text fields, one filename hyperlink, fullscreen Expand and
compact Restore, and absence of new local draft files. This caught a real empty
paragraph serialization bug that repeated the next line, including Drive links.

Actual WebView2 diagnostics measured a short newsletter at 615 GTK units and a
9,000-CSS-pixel synthetic newsletter at 13,778 GTK units on the fractional-DPI
host. Both hid their native child beneath Settings and restored after closing it.
WebView2 emits a nonfatal native window-class cleanup diagnostic at shutdown.
Its InPrivate runtime metadata used approximately 1.5–2.6 MB in these synthetic
runs; this overhead is separate from the disposable body budget. Linux uses an
ephemeral WebKit session instead. No private email fixtures are published.

Local checks passed strict Clippy, cache corruption/binary overhead, legacy draft
safety, and migration to Gmail drafts with preserved custom polling preferences.
Linux CI exercises the new-reply unread update without replacing loaded widgets,
plus actual native and AppImage HTML/media with the mail cache disabled. Results
must be recorded from the completed run before claiming the new AppImage passed.

[Linux run 40247171247](https://github.com/SleepyZzed/luma-mail/actions/runs/40247171247)
passed actual WebKit layout, colours, hyperlink, prepared image and inert-content
checks, followed by real RAM-backed WAV/WebM playback. This exposed and fixed a
GStreamer registry path conflict with WebKit's sandbox device mounts. Registry
metadata now stays in a private runtime directory, outside the message-cache
budget (the initial codec inventory cannot be disabled with the update flag).
The run then failed in the diagnostic screenshot path; capture now uses WebKit's document snapshot API.
Packaged-engine acceptance is still required before publishing the new AppImage.

## Completed Linux native and AppImage checks — 26 October 2026

[Run 40249509620](https://github.com/SleepyZzed/luma-mail/actions/runs/40249509620)
passed strict formatting/Clippy, 84 core tests, four desktop unit tests and the
separately executed GTK/D-Bus acceptance test. Composer diagnostics passed initial
To focus, completion, Tab/Shift+Tab, filename hyperlinks, Expand/Restore and absence
of new local drafts. The new-reply test preserves the existing row and reader
while restoring unread state.

Both the native release and actual bundled AppImage passed sandboxed WebKit
layout/colour/link/image/inert-content checks and RAM-backed WAV/WebM playback.
Their captured newsletter PNGs were inspected and have identical SHA-256
`bdf19a20ecc5898b61471330b19f2d3111bfc284040816dc743647cedbb3e8c9`.
The bundle also passed GTK launch and dconf/session cursor preference checks.

With cache set to zero, neither run created a body-cache directory, attachment
spool or local composer store. Codec inventories were 1,037,145 bytes native and
1,041,393 bytes bundled, containing plugin metadata in private runtime storage.
The bundle includes distro notices and corresponding source-package links.
These synthetic checks do not replace the remaining target-desktop/live-account
checks listed in STATUS.md.

[Release run 40250308268](https://github.com/SleepyZzed/luma-mail/actions/runs/40250308268)
repeated the gates on merged source `83c8c0412c10b432b7cb93566b1825dbd6d1adf9`,
including the sign-in registration guard, and published
[v0.2.0-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.2.0-dev.1)
as a prerelease. The actual downloaded 157,092,344-byte AppImage matched both
the published SHA256SUMS and GitHub asset digest:

```text
cbd1b7d8933ca684d61e1d06271693f27711fcf97538b2f6ddfcfcd7150fcafe
```

The release's native and AppImage newsletter snapshots also match the inspected
PNG digest above. Source archives, checksums and native dependency source links
accompany the AppImage. Post-publication documentation updates do not change
the tagged application sources or binary.

## AppImage missing-system-WebKit regression — 26 October 2026

A target Linux report exposed a packaging gap in 0.2.0: its helpers could fall
back to the CI host's WebKit libraries, while relocated relative bubblewrap bind
destinations depended on Ubuntu's directory layout. On CachyOS, the helper failed
to load `libwebkitgtk-6.0.so.4`, then the D-Bus proxy aborted with a read-only bind
destination. Plain text avoided the HTML engine and remained usable.

[Run 40305224654](https://github.com/SleepyZzed/luma-mail/actions/runs/40305224654)
reproduced the published image's failure with the runner's WebKit/JSC libraries
and helper directory hidden. Three sandbox-launcher tests passed. The corrected
AppImage passed X11 HTML/media without system WebKit and completed Wayland HTML
and media diagnostics too. That run failed during test-runtime cleanup of the
document portal's FUSE mount; the fixture now detaches its own mount before
cleanup. Final 0.2.1 publication requires the complete gates, including a
read-only extracted tree. This is not evidence of actual FUSE/target-GPU testing.

[Release run 40306946881](https://github.com/SleepyZzed/luma-mail/actions/runs/40306946881)
subsequently passed all gates on exact head
`b61c768103ef091c7d0984c195c8962d76156d81`: strict fmt/Clippy, 84 core tests,
four desktop tests, separately executed GTK/D-Bus acceptance, three launcher
tests, composer and cursor checks, native and bundled HTML/media on X11 and
Wayland, absence of host WebKit/JSC/helpers, and the read-only extracted tree.
All six newsletter PNGs have the inspected digest
`bdf19a20ecc5898b61471330b19f2d3111bfc284040816dc743647cedbb3e8c9`.
Zero-cache body/attachment/local-draft retention checks also passed.

It published [v0.2.1-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.2.1-dev.1)
after the complete gates. The downloaded 157,288,952-byte AppImage matched both
SHA256SUMS and GitHub's asset digest:

```text
23b647b70e2a0696f177e2f2ececb3427e18f18029b5c552a37e27d3c9359e31
```

The tested source was merged in PR #2. The corrective bundle replaces the 0.2.0
download; it does not claim hardware/FUSE or Google verification is complete.


## HTML layout and theme regression — 26 October 2026

The reported GitHub Actions notification was inspected read-only. Its inline
24-pixel Status column lost `box-sizing`, while reader-wide aggressive wrapping
allowed words to shrink to one-letter columns. The HTML title also leaked into
the visible body. Sanitization now preserves box sizing, removes title content
and keeps complete words in table cells. A synthetic equivalent is committed;
private message HTML and user screenshots are not published.

Neutral HTML surfaces/text follow the active GTK/Noctalia palette. Original
colours restores sender styling for the opened message; brand buttons and image
pixels keep their colours. Theme changes reuse bounded prepared images in RAM.
Inline images fit the reader width and 480-pixel height. Small decoded images
are no longer enlarged to fill the preview bounds.

[PR run 40313390992](https://github.com/SleepyZzed/luma-mail/actions/runs/40313390992)
passed the complete Linux gates on `d048e4ee9b498003659703066225a736d7d86304`.
[Release run 40313692368](https://github.com/SleepyZzed/luma-mail/actions/runs/40313692368)
repeated them on exact tagged source `4e00f3b0eea00228ccb826b0b9c4bbb7e2b118b4`,
including the updated source-package pin and sign-in registration guard:

- Strict fmt/Clippy, 86 core tests, four desktop tests, separately executed
  GTK/tray/notification D-Bus acceptance, and three sandbox-launcher tests.
- Composer focus/Tab/Shift+Tab, recipient completion, hyperlinks, compact Restore,
  Gmail-only draft storage, and bundled cursor settings.
- Real WebKit table/title/image-height checks, dark/light theme updates and the
  Original colours toggle, plus in-memory WAV/WebM playback.
- Native X11/Wayland and bundled X11/Wayland, with host WebKit/JSC/helpers hidden,
  and a read-only extracted AppImage tree. Sandbox protections remain enabled.
- Zero-budget runs created no body cache, attachment spool or local draft store.
  Private runtime codec inventories remained about 1 MiB and contain no mail.

Final dark document snapshots match across all six renderer environments:
`0d7f105d8a2e3fe1ef2ab4fcd4c048f9d0d7b3a4fde80b06a3b9b9b742373d23`.
Light snapshots match `9899f7185408204deb00e83a88731fb2738a2e137681492f6928c120f55fa7a4`;
original-colour snapshots match `e3a0bc1a80936bec94a8e6faf4e1e7d6a0ea8ce617dc40cfb9730153b18dcd39`.
The captures were inspected. Windows QA also checked the actual notification's
1471-unit content height and Settings stacking, and a synthetic table's live
Noctalia palette changes and Original colours toggle.

It published [v0.2.2-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.2.2-dev.1)
as early access. The downloaded 157,411,832-byte AppImage matches the published
SHA256SUMS and GitHub asset digest:

```text
2cd2aab31fdebe4ed7d75aa0f3c3e60790af821f4038d627b87cc7393f91276e
```

These checks do not claim actual target GPU/FUSE, two-account Linux acceptance
or completed Google verification. PR #3 contains the changes. Later documentation
commits do not alter the tagged source or published binary.

## 0.3.1: presentation, installed fonts and safe bulk selection — 26 October 2026

[PR #4](https://github.com/SleepyZzed/luma-mail/pull/4) adds a frameless main window,
pinned Luma/Compose header and Settings/menu footer, independently scrolling
navigation, 75–200% UI scaling and finite eased transitions with reduced motion.
It adds installed-family composition without font downloads/copies and bounded
visual restoration of new Luma-generated rich drafts. Unknown/older HTML stays
intact in source mode. Font sizes sent in mail remain independent of UI scaling.

Ctrl+click toggles loaded conversations; Shift+click selects ranges. Select offers
Select all loaded, Clear, counts and bulk actions. Select all never loads pages.
Context actions use the selection. Reply, forward, draft editing, sender blocking
and label administration are hidden and handler-guarded for multiple selections;
an empty bulk selection cannot fall back to the opened conversation.

Local Windows checks passed formatting, strict Clippy, 88 core tests and five
desktop tests. Native diagnostics exercised font choice/further typing, visual
rich-draft round trips, To/Tab/Shift+Tab/Expand/Restore, blue links, reduced motion,
Ctrl/Shift/Select all, incompatible menu/handler guards and fixed sidebar geometry
at 100%/150%. Actual UI inspection confirmed bulk menus and hidden reader actions.
Microsoft Times New Roman was chosen/typed in the Windows font picker; Linux
families depend on what the user has installed. The different preview accents came
from isolated test settings, not a forced change to existing users' palettes.

[Final release run 40330041851](https://github.com/SleepyZzed/luma-mail/actions/runs/40330041851)
passed on exact tagged head `a513ec1c5cc57bfe152f656091925b328fc03339`:

- Strict fmt/Clippy, 88 core tests and five desktop tests; separate real GTK/tray/
  system-notification D-Bus acceptance and three sandbox-launcher tests.
- Composer font/typing/rich-draft/navigation/scaling checks and new selection/
  fixed-sidebar diagnostics. Normal composition created no local draft store.
- Native and bundled WebKit/GStreamer on X11 and headless Wayland, including
  hidden host WebKit/JSC/helpers and a read-only extracted AppImage tree.
- Real table widths, title removal, inline-image bounds, theme/original-colour
  changes, UI zoom and settled full-content heights; stale layout results rejected.
- In-memory WAV/WebM decoding/playback and host cursor-settings transport.
- Zero-cache runs created no body cache, attachment spool or local draft store.
  Private codec inventories remained approximately 1 MiB and contained no mail.

Published dark/light/original document snapshots retain the 0.2.2 hashes above;
the final dark/light captures were downloaded and visually inspected. Presentation
changes avoid rebuilding existing rows or refetching Gmail. Remote search,
25-row metadata pages, at most 1,000 loaded rows, on-demand bodies/previews and
existing allocation-budget/zero-cache behavior remain. Selection stores only
loaded keys in memory. There is no new mailbox index or attachment/draft archive.
This is a boundedness audit and synthetic validation, not sustained mailbox/GPU
performance acceptance.

[v0.3.1-dev.1](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.3.1-dev.1)
is early access. Its downloaded 157,469,176-byte AppImage matches SHA256SUMS and
GitHub's asset digest:

```text
b6c82e243b2111231c5fcd07251bcf36eceeb26abd2ffd66442f1d655ced2b66
```

The Arch package pins functional source `5494b9f9a4ccd0dcb5de22fdbe23152be3560396`;
its downloaded source archive matches
`6b8d7adc37ccefba8c5310b343d7da7d82758fbb25a88030663945ef86394d6c`.
PR #4 is merged. Later verification-document updates do not change the tagged
binary. Target Hyprland/GPU/FUSE, two-account Linux/Noctalia, real giant-mailbox
profiling and Google verification remain gates; see roadmap and release checklist.
