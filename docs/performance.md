# RAM, CPU and storage acceptance

Goals are below 150 MiB at idle and 200 MiB in ordinary use where feasible,
with idle CPU substantially below 0.4% of one core. These are measurement goals,
not enforced process-kill thresholds. Large media/attachments and graphics
drivers can add memory. Accurate HTML, previews and active composition are kept.

0.7 changes:

- GtkListView recycles inbox widget trees instead of retaining a widget tree
  per loaded conversation. Native tests load 1,000 metadata records, scroll to
  both ends, retain offscreen Ctrl/Shift selection, update a recycled row and
  select all loaded records. The tested GTK build bound 205 row trees.
- Clicked rows carry their model identity directly. Selection uses a bitset;
  sorting and refresh restore selection by account/thread key.
- HTML resize measurements run on allocation events, not a permanent frame
  callback. Live views share weakly held ephemeral engine resources. Closing
  the last view releases their owners. Back/forward caching is disabled.
- Inactive readers release their bodies and renderer views after 45 seconds
  hidden. Composers and open attachment previews are protected. Restore reloads
  only the selected conversation, without rebuilding the mailbox list.
- Immutable MIME bodies and retained attachments share buffers across callbacks
  and draft snapshots. Autosave conflict checks project only the current message
  ID instead of re-fetching the entire draft body.

`packaging/test-performance.sh` profiles the actual release AppImage under a
private D-Bus/X11 session. Its JSON includes the main process and all descendant
WebKit, sandbox and helper processes. It measures launch, a 1,000-row inbox,
scrolling, one/three HTML messages, release and background reclamation. RSS,
proportional resident memory (PSS), private memory and CPU are reported separately.
Unreadable sandbox PSS is explicitly marked incomplete rather than counted as
zero. Shared libraries make summed RSS larger than physical memory consumption.
Software-rendered CI is not a substitute for a real GPU/Hyprland desktop.

For a three-hour test while the actual Linux app syncs and is used normally:

```sh
python3 packaging/profile-processes.py --attach "$(pgrep -n -x luma-mail)" 10800 live-performance.json
```

Run from the source directory, with exactly the intended Luma instance running.
The profiler reads process counters, not mail, tokens, window titles or arguments.
It never stops or reconfigures the app. The report is an explicitly saved test
artifact, not an automatic app cache. Repeat with Inbox scrolling, large mail,
composition and tray restore. Several-hour real sync/desktop acceptance remains
a 1.0 gate; a short synthetic CI run does not establish it.

Disk categories remain explicit in `docs/storage.md`: installed AppImage/native
libraries, required preferences/secure credentials, optional bounded mail cache,
and temporary graphics/codec metadata. A zero mail-cache budget creates no
mail-body, draft or attachment archive. User-requested saves are separate files.
