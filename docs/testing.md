# Verification and manual acceptance

Automated tests use synthetic Gmail responses and a local mock HTTP server.
They never authenticate to or mutate a real Google account. Run all checks in
the README. Linux CI also runs a native launch under Xvfb and a private D-Bus
session. See `verification.md` for the executed run and results.

The Linux-only desktop acceptance test exercises the real GTK close handler,
application lifetime hold, SNI tray activation, D-Bus menu Quit, and GIO delivery
to `org.freedesktop.Notifications.Notify` while hidden. Its test service is an
isolated protocol fixture, not an application notification UI. No email is
simulated. It does not establish how Noctalia displays or routes an alert.
To run it on Debian/Ubuntu with `python3-dbus`, `python3-gi`, Xvfb and D-Bus:

```sh
dbus-run-session -- xvfb-run -a /usr/bin/python3 packaging/test-desktop.py
```

The ignored Rust acceptance test is compiled only in the test executable. It is
absent from the shipped application binary and uses a temporary configuration.
The GTK check also verifies that a missing OAuth client opens one guided setup
window, repeated Add clicks reuse it, background changes retain loaded pages
and a bulk selection, and unread/sidebar updates keep label navigation.

The bundled-settings check runs the AppImage on a private D-Bus session with
isolated configuration, writes a cursor preference through the host dconf
service, and checks the actual GTK settings inside the bundle. It then checks
session cursor overrides. It verifies configuration transport, not cursor
artwork or fractional scaling on the real compositor.

## Real Gmail tests (deliberate user actions)

Use disposable test messages in two accounts. Configure your OAuth app and
finish consent yourself. These steps modify messages only when you click the
corresponding action in Luma. Do not use production messages for destructive
tests. Permanent message deletion is not exposed.

1. Connect two accounts. Restart. Confirm credentials load from Secret Service,
   both individual inboxes work and account source is visible on every row.
2. Open Unified Inbox, All Mail, Unread, Starred, Sent, Drafts, Archive and Trash.
   Compare with the same queries in Gmail. Confirm Spam/Trash do not leak into
   All Mail. Matching thread subjects in separate accounts must stay separate.
3. Inspect API traffic or request logs with **content/token redaction**. Initial
   list loading must not request raw messages, attachments or every page. Click
   Load more once; verify only the next needed page is retrieved.
4. Click a conversation, expand an old message and inspect threading, recipients
   and body order. Other bodies must remain unfetched until expanded.
5. For a test thread, archive, refresh Gmail in a browser, and confirm Inbox was
   removed while mail remains in All Mail. Test Trash and restore similarly.
   Test read/unread, stars, label add/remove and label CRUD. Select test rows
   across accounts and verify each operation reaches the correct account.
6. Search for a years-old message using Gmail operators. It must appear without
   historical local downloading. Change the same message from Gmail and refresh
   Luma; server state should win.
7. Compose/reply/reply-all/forward test messages to yourself. Check the generated
   Message-ID/In-Reply-To/References, account and Gmail thread association. Test
   To/Cc/Bcc and UTF-8 subjects. Try invalid recipients and header injection.
8. Type a draft, wait for Saved, close/restart and open Drafts → message → Edit
   Gmail draft. Modify it in Gmail concurrently and confirm Luma refuses to
   overwrite a changed message ID. Drafts whose attachments exceed the editing limit are refused
   for editing rather than stripping their attachments. Smaller attachments must survive edit/save/reopen. Concurrent edits still
   have a residual check/write race because Gmail offers no draft CAS API.
9. Attach files below 16 MiB total. Verify encoded MIME arrives intact. Download
   selected received attachments to a chosen directory. Confirm no historical
   attachments have appeared in the cache.
10. Enable Drive consent. Upload a large test file and explicitly share it with
    a test recipient. Observe real progress; open the link as that recipient.
    Test cancellation, offline interruption, failed permission creation and
    Workspace sharing restrictions. No link should be inserted on failure.
11. Leave Luma running and hide its window with Close to tray enabled. Send yourself a test email. Confirm notification/account,
    privacy mode and click-to-open. Restart: no old-mail notification storm.
    Change polling interval and per-account toggle; check no duplicate alerts.
    Test a burst larger than 25 messages: later checks must continue the batch.
12. Test the Noctalia instructions on the installed version: wallpaper, app
    light/dark mode, atomic palette replacement, invalid palette and removal.
13. Start with cache disabled. Record `du` for the cache/config. Connect an account
    with 100,000+ messages, page a few times and search old messages. Cache must
    stay empty; settings growth must depend on accounts/preferences, not mail.
    Enable a 50 MiB cache, open enough messages to exceed it and verify eviction
    and expiry. Test custom limits, shrink to zero and per-account clearing.
14. Disconnect internet, revoke consent, delete a selected thread externally,
    suspend/resume and test slow responses. Confirm useful errors, no stuck
    duplicate send attempts, no optimistic local-only changes and no hidden
    process after full exit.
15. Verify keyboard navigation, tooltips, screen reader, HiDPI, fractional scaling,
    Wayland file picker, mailto handling and resizing on CachyOS/Hyprland.
16. With no OAuth JSON configured, Add Google account must explain the setup
    prerequisite. Import a Desktop client locally and confirm the default
    browser opens; test reopen/copy-link, cancellation and timeout recovery.
17. Verify the normal system cursor and size in Luma, including user-local theme
    files. Change the configured desktop cursor while Luma runs; inspect the
    effective cursor row in About & Diagnostics.
18. Use Search options & accounts for date/attachment filters and a subset of
    three accounts; compare the exact query in Gmail. Test Ctrl+1…9. Apply a
    label to selected test conversations across accounts where same-named
    labels have different IDs; a missing match must be reported without change.

## Development host boundary

This delivery is developed on Windows with a portable MSYS2 Rust/GTK toolchain
under the task's `work/` directory, separate from the source deliverable. Native
Linux Secret Service, Noctalia, Wayland, notifications and real Google accounts
must be verified on Linux. Windows builds deliberately reject account login;
they do not provide a plaintext substitute keyring.
