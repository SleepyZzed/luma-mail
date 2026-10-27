# Luma Fluid (in progress)

Custom Qt Quick frontend for the existing online-first Rust mail engine. This is
not the released client yet: rich composition, supported photos and private HTML
reading are wired, while remaining integration/parity and resource gates are
still in progress. Keep using the 0.9 release for normal mail.

- [Wiring and parity gates](../../docs/ui-migration.md)
- [Architecture/storage boundary](../../docs/fluid-architecture.md)
- [Build and native checks](../../docs/fluid-validation.md)

`--design-preview` uses isolated, synthetic metadata with networking disabled.
`--preview-reader --preview-account --smoke-test` captures an example split view
when `LUMA_SMOKE_SCREENSHOT` specifies an output path. These flags do not sign in
or read the user's mailbox. `--motion-test` validates the real native scene and
private engine protocol without claiming functional parity for pending features.
