# Verification log

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
