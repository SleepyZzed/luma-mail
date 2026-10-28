# Fluid frontend parity audit

The replacement packages run the Qt Quick interface and the existing Rust engine;
they do not run the GTK frontend. The GTK source/tag remains a reference until
target-desktop acceptance passes. This is implementation coverage, not a claim
that every real-account/desktop acceptance criterion has passed.

| Existing functional group | Replacement and current verification | Remaining acceptance |
| --- | --- | --- |
| Unified/account folders, collapsed disclosures, labels and loaded selection | Sidebar/Inbox, Rust views/actions; native navigation, selection and label tests | Real two-account routing and target Linux desktop |
| Archive/Trash/read/star/importance, Undo, bulk rules and sender filters | Same Rust provider actions; navigation/sync tests; working pointer and keyboard context menus | Account-isolated live actions and recovery |
| Online search, filters, pagination and highlighting | SearchSheet and bounded native model; remote search/native search tests | Giant live mailbox and quota behavior |
| Thread navigation, message actions, HTML and privacy/image choices | Reader and private platform adapters; HTML/2,503-header/recovery tests | Hyprland/GPU, real newsletters and recipient interoperability |
| Attachments, previews, media, exports, file drops and clipboard pictures | RAM-only preview/thumbnail providers and existing core transports; native attachment/composer tests | Real clipboard/portals, codec availability and Drive recipient access |
| Compact/full/inline/pop-out composer, fonts, formatting, lists, links and tables | One retained editor/session, native rich document and spring geometry; composer/pop-out/document tests | Real MIME delivery and external-HTML editing acceptance |
| Gmail drafts, explicit close policy, conflicts, cancellation and uncertain delivery | Same serialized session/provider operations; composer/conflict/transport tests | Two-account live conflicts, interruption and restart |
| Verified identities, recipient suggestions and online rich signatures | Existing sender/signature/recipient core; native composer/signature tests | Alias delivery and signature rendering for another recipient |
| Google login and optional permissions, reconnect and account management | Existing registration, OAuth, system vault and native sign-in/settings; auth tests | Fresh combined-consent denial/reconnect on two accounts; Google approval |
| Available profile photos | Same bounded supported Contacts/connected-account lookup and RAM provider; photo tests | Real two-account permission/photo refresh; Google does not expose arbitrary private Gmail avatars |
| Tray, system notification transport, scaling/theme, shortcuts and window lifecycle | Native Qt/D-Bus/Windows adapters and event-driven Noctalia settings; native shell and platform component tests | Real notification appearance/click, Hyprland/Noctalia, fractional scaling and suspend/reconnect |
| Online-first storage and resource bounds | Existing core cache enforcement; zero-cache, model/lifecycle and installed/runtime audits | Sustained real-mailbox resource audit; Linux idle/HTML RAM targets are still unmet |

The 28 October feedback pass adds centered/padded action buttons while keeping
navigation labels aligned, theme-owned animated menus/dialogs, pointer and keyboard
context actions, lazy editor-menu construction, and pop-out bounds/corner springs.
Changing conversations closes an inline reply through the configured draft policy.
An unsaved reply is kept in RAM while Ask is answered; Keep editing retains it as a
floating card. A detached composer stays open. Input never schedules a Gmail draft.

Roadmap work already implemented is distinct from remaining authoring features:
interrupted Drive sharing/upload recovery/existing-file selection remain open.
These were outstanding before the migration and have not been silently marked
complete. Full accessible focus/keyboard acceptance and independent
security/privacy review also remain release gates.

See ui-migration.md for the GTK callback-to-backend map, fluid-validation.md for
exact build evidence, roadmap.md and RELEASE-CHECKLIST.md for 1.0 acceptance.

The 0.11 batch implements bounded readable whole-conversation forwarding,
reply/forward mode switching and RAM-only CID inline-picture authoring.
See v1-acceptance-0.11.md for the updated limits
and original-criteria audit; live delivery remains distinct from fixture coverage.
