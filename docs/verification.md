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
