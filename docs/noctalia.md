# Noctalia palette integration

The development host is Windows and has no installed Noctalia. Its installed
version could not be inspected. Run `noctalia --version` on your Linux machine
and follow the matching instructions below. These configurations use documented
template mechanisms, not reads of Noctalia's internal palette files.

## Noctalia v5+

```sh
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/noctalia/templates"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/luma-mail"
cp data/noctalia/luma.toml.in "${XDG_CONFIG_HOME:-$HOME/.config}/noctalia/templates/luma.toml.in"
```

Add to a Noctalia config overlay, for example `~/.config/noctalia/luma.toml`:

```toml
[theme.templates.user.luma]
input_path = "$XDG_CONFIG_HOME/noctalia/templates/luma.toml.in"
output_path = "$XDG_CONFIG_HOME/luma-mail/noctalia.toml"
```

Noctalia expands the leading XDG tokens using its defaults. Defining the entry
enables it. Apply the theme through Settings → Media & UI → Theme. Select
Follow Noctalia in Luma Settings → Appearance. The template uses `default`
colour roles and `mode`, which follow the application theme mode rather than
`shell_mode`. Builtin, wallpaper, community and custom sources use the same
resolved token map.

Luma monitors the **parent directory**, including atomic replacement of the
generated file, through GIO's filesystem monitor. It validates all colours and
text contrast, then reloads CSS without a polling daemon. Invalid/missing files
fall back to the system theme. No post-hook is needed.

## Legacy Noctalia Shell v4

Enable User templates in Settings → Color Scheme → Templates → Advanced.
Register the same input file in `~/.config/noctalia/user-templates.toml`:

```toml
[config]

[templates.luma]
input_path = "~/.config/noctalia/templates/luma.toml.in"
output_path = "~/.config/luma-mail/noctalia.toml"
```

Resolve custom XDG paths explicitly for v4 if needed. Apply the palette and
select Follow Noctalia in Luma. Verify that your legacy template engine resolves
`{{mode}}` to `dark` or `light`; a missing/unsupported token causes a visible
fallback rather than applying unvalidated CSS.

## Verify

Change wallpaper with wallpaper-derived colours enabled. Inspect the generated
`luma-mail/noctalia.toml` and observe Luma updating without restarting. Repeat in
light mode, with shell mode pinned independently, and with another palette
source. Live integration still requires verification on the target desktop.

References checked on 2026-10-25:

- [v5 App Theming](https://docs.noctalia.dev/noctalia/theming/app-theming/)
- [v5 Template Reference](https://docs.noctalia.dev/noctalia/theming/templates/)
- [v5 app versus shell mode](https://docs.noctalia.dev/noctalia/theming/)
- [Legacy v4 User Templates](https://docs.noctalia.dev/noctalia-shell-legacy/theming/user-templates/)
