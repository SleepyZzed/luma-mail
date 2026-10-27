# Next-build custom frontend

Requested after the 0.9 workflow completes. Do not change the current layout or
framework in 0.9. Replace the GTK frontend with a distinctive shared interface;
retain the Rust mail/provider engine, OAuth, Gmail operations, cache enforcement,
preview/parsing code and other non-UI functionality wherever possible.

The visual direction is a restrained modern interface with fluid morphing
transitions and clear action feedback. Keep account folders, conversation lists
and available profile pictures. Explore a right-side composer that slides up and
returns smoothly, including stable intermediate resize/maximise states. Treat
the supplied concept as direction, rather than a layout to copy. Linux remains
primary and borderless for Hyprland; Windows retains title-bar controls.

Use a portable design-token layer, including Noctalia v5 colour updates, instead
of inheriting arbitrary GTK control appearance. Prototype and measure a candidate
frontend before committing the application to its rendering/runtime overhead.
Replacing the UI requires reimplementing UI behaviour; it does not require
throwing away the mail engine. Keep one feature implementation across platforms.

Parity gates include all folders/accounts/search/pagination, modifier selection
and bulk/context actions, sender controls and labels, profile pictures, threaded
reading and privacy-safe HTML/images, attachment/document/media previews,
clipboard images, Drive upload/sharing/progress/cancellation, rich composition,
fonts/colours/lists/links, recipient chips/completion and keyboard navigation,
compact/expanded/inline/pop-out composition, bounded undo/redo, Gmail drafts and
conflict handling, verified sending identities, online rich signatures, settings,
UI scaling/reduced motion, tray operation, real OS notifications and secure vaults.

Preserve online-first behaviour: no mailbox mirror/index, attachment archive,
local autosave or downloaded font store. Keep metadata/pages, histories, previews
and request concurrency bounded. Stop animations and layout work when idle;
respect reduced motion. Count all renderer/helper processes and whole-package
installation size separately from the cache. The original RAM targets remain
aspirational gates until measured achieved; a frontend change is not proof of
lower memory use. Complete Gmail 1.0 acceptance before adding other providers.
