# Themed reader presentation and composer layout

The HTML surface stays hidden until the current navigation has completed and
its content height has been measured. A retargetable spring expands the loading
pill's clipping bounds into the visible document. The browser keeps its final
layout width throughout this reveal; the animation does not wrap its contents
on every frame. Reduced motion reveals the ready document immediately.

On Windows, both the native host's erase/resize paint and WebView2's creation-time
and controller backgrounds use the current theme. The host has no diagnostic
caption. Microsoft documents a startup flicker when only the post-creation
background property is set; Luma also supplies the creation-time background.
See [WebView2's background documentation](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2controller.defaultbackgroundcolor).
On Linux, the off-record WebEngine surface uses the theme background and stays
transparent to the scene until the matching navigation is measured. Navigation,
measurement and image results are rejected after replacement or reader close.
The explicit Original colours option still preserves a sender's colours.

The engine can emit the inert themed text/layout before image servers finish.
It fetches at most two image sources concurrently and shares the existing single
preview-decoder lease with attachment previews. A cancelled task cannot release
that lease while its blocking decoder is still running. Existing limits remain:
eight sources, four MiB aggregate rendered image data, input/pixel limits, and
network deadlines. No mailbox, image archive, index or local draft store is added;
the existing optional original-body cache and its zero/default/custom budgets
remain unchanged. Message collapse/close releases rendered assets.

The composer now lays out its controls at the destination dimensions while the
card's spring-driven bounds clip/reveal them. Previously those controls inherited
the changing card dimensions and repeatedly reflowed during opening and expansion.
The editor is still created on demand and destroyed after closing settles.
Parked/offscreen layout motion does not count as an already-open composer;
opening still starts its morph even during a window-layout transition.

Coverage includes native background pixels and clipping without viewport resize,
startup visibility, measured-height readiness, stale image cancellation, bounded
image concurrency/data, controller cleanup, editor layout stability, rich editing,
pop-out/docking and reduced-motion/idle spring checks. Timing reports describe the
test machine and do not guarantee a frame rate on every GPU. Target Hyprland/GPU
and sustained resource acceptance remain separate 1.0 gates.
