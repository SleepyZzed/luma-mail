# Luma visual system

The 0.7 visual layer uses 52 app-owned outlined SVG symbols, bundled in the
resource rather than taken from the installed icon theme. The same 24-unit grid,
1.7-unit round strokes and optical sizes cover navigation, mail actions,
attachments and editing. GTK symbolic recolouring follows the active palette.
No icon font, web assets, downloads or background animation loop is added.

The three panes, fixed navigation areas, compact/expanded/inline composer,
keyboard actions and mail functions are retained. Controls use subdued tonal
surfaces, stable inset focus indication, compact subject height and small
borderless disclosures. GtkExpander's root is `expander-widget`; its `expander`
node is just the arrow. Styling that arrow as a padded card caused the large
boxes visible in earlier Linux builds.

The supplied Luma wordmark is traced into real vector paths with gradient fills
in `data/luma-wordmark.svg`. A standalone star mascot is drawn in
`data/io.github.lumamail.Luma.svg` for the desktop/AppImage icon and tray.
The pinned sidebar uses a 960-pixel transparent export so it does not parse the
detailed trace on every launch. The SVG sources remain editable and font-free.

The inbox uses GtkListView factory recycling. Its account/thread model and
selection remain independent of widget bindings. Clicked rows carry their own
identity; no widget-map scan is needed. The native 1,000-row diagnostic also
checks offscreen range selection, updates, Select all and scroll recycling.

GTK remains the native toolkit. Replacing it would require rebuilding the
working Linux integration and increase the package/runtime footprint before
acceptance. The redesign is checked using native GTK screenshots on Linux,
including the reader, compact composer and inline reply, in addition to Windows
interaction diagnostics. Synthetic screenshots use an isolated warm palette;
real installations retain their own desktop/Noctalia colours.

Linux windows are resizable and undecorated, without client resize borders.
The compositor owns borders, placement and resize bindings. The mail context
menu does not claim Super/Meta + right-click. No Hyprland configuration is
changed. The actual resize binding still needs target-desktop acceptance.
See [Hyprland's mouse bindings](https://wiki.hypr.land/Configuring/Basics/Binds/).

UI scaling and reduced motion apply to the replacement controls. Animations
remain finite and render only during an interaction; no mail is fetched for
visual effects. System pointer settings continue to be used.
