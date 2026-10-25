# Verification log

Development date: 2026-10-25.

Host: Windows; portable MSYS2 UCRT64 toolchain installed inside the task's
`work/toolchain` directory. Rust 1.99.1, GTK 4.24.1, libadwaita 1.10.0;
gtk4 Rust bindings 0.11.5, libadwaita bindings 0.9.2, Reqwest 0.13.5.

Checks being finalized for this delivery:

- `cargo fmt --all --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace`
- `cargo build --locked --workspace`
- `cargo build --locked --release --workspace`
- Native GTK launch and automatic exit with `--smoke-test`.

The preview PNG is rendered from the running GTK window through GTK/GSK, using
an isolated test configuration/cache directory. No account fixture is injected.

Not executed: Linux compilation/runtime, real Google OAuth consent or Gmail/Drive
operations, actual Secret Service D-Bus interaction, Noctalia/Hyprland/Wayland
integration, external notification delivery, makepkg/AUR publication, GitHub CI.

See `testing.md` for the manual test procedure and `STATUS.md` for outstanding
features. A passing local build is not a substitute for those acceptance checks.
