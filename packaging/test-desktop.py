#!/usr/bin/env python3
"""Exercise actual GTK close/restore/quit and notification D-Bus transport.

Run under dbus-run-session -- xvfb-run -a. The fixture implements the desktop
protocol endpoints; it does not simulate incoming email or ship in the app.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import time

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()
loop = GLib.MainLoop()
state = {"service": None, "notification": False, "activated": False, "quit": False}


class Watcher(dbus.service.Object):
    @dbus.service.method("org.kde.StatusNotifierWatcher", in_signature="s", out_signature="")
    def RegisterStatusNotifierItem(self, service):
        state["service"] = str(service)

    @dbus.service.method("org.freedesktop.DBus.Properties", in_signature="ss", out_signature="v")
    def Get(self, interface, prop):
        assert interface == "org.kde.StatusNotifierWatcher"
        if prop == "IsStatusNotifierHostRegistered":
            return dbus.Boolean(True)
        if prop == "ProtocolVersion":
            return dbus.Int32(0)
        if prop == "RegisteredStatusNotifierItems":
            return dbus.Array([state["service"]] if state["service"] else [], signature="s")
        raise dbus.exceptions.DBusException("unknown property")


class Notifications(dbus.service.Object):
    @dbus.service.method("org.freedesktop.Notifications", in_signature="", out_signature="as")
    def GetCapabilities(self):
        return ["body", "actions"]

    @dbus.service.method("org.freedesktop.Notifications", in_signature="", out_signature="ssss")
    def GetServerInformation(self):
        return ("Luma CI fixture", "Luma", "1", "1.2")

    @dbus.service.method("org.freedesktop.Notifications", in_signature="susssasa{sv}i", out_signature="u")
    def Notify(self, app, replaces, icon, summary, body, actions, hints, timeout):
        assert summary == "Luma desktop integration test"
        assert root.joinpath("hidden").is_file(), "notification must arrive after close-to-tray"
        state["notification"] = True
        return dbus.UInt32(1)

    @dbus.service.method("org.freedesktop.Notifications", in_signature="u", out_signature="")
    def CloseNotification(self, notification_id):
        pass


watcher_name = dbus.service.BusName("org.kde.StatusNotifierWatcher", bus)
watcher = Watcher(bus, "/StatusNotifierWatcher")
notification_name = dbus.service.BusName("org.freedesktop.Notifications", bus)
notifications = Notifications(bus, "/org/freedesktop/Notifications")

with tempfile.TemporaryDirectory(prefix="luma-desktop-test-") as temporary:
    root = Path(temporary)
    env = dict(os.environ, LUMA_TEST_ROOT=str(root / "app"), LUMA_DESKTOP_TEST_ROOT=str(root))
    process = subprocess.Popen([
        "cargo", "test", "--locked", "-p", "luma-mail", "desktop_dbus_end_to_end",
        "--", "--ignored", "--test-threads=1", "--nocapture"
    ], env=env)
    deadline = time.monotonic() + 120
    errors = []

    def tick():
        try:
            if process.poll() is not None:
                assert process.returncode == 0, "GTK integration test process failed"
                assert all(state[k] for k in ("notification", "activated", "quit")), state
                loop.quit()
                return False
            assert time.monotonic() < deadline, "D-Bus acceptance test timed out"
            if state["service"] and state["notification"] and not state["activated"]:
                item = bus.get_object(state["service"], "/StatusNotifierItem")
                props = dbus.Interface(item, "org.freedesktop.DBus.Properties")
                menu_path = props.Get("org.kde.StatusNotifierItem", "Menu")
                menu = dbus.Interface(bus.get_object(state["service"], menu_path), "com.canonical.dbusmenu")
                _, layout = menu.GetLayout(0, -1, [])
                entries = {str(node[1].get("label", "")): int(node[0]) for node in layout[2]}
                assert "Open Luma Mail" in entries and "Quit" in entries, entries
                state["quit_id"] = entries["Quit"]
                state["menu"] = menu
                dbus.Interface(item, "org.kde.StatusNotifierItem").Activate(0, 0)
                state["activated"] = True
            if root.joinpath("restored").is_file() and not state["quit"]:
                state["menu"].Event(state["quit_id"], "clicked", dbus.Int32(0, variant_level=1), dbus.UInt32(0))
                state["quit"] = True
        except Exception as error:
            errors.append(error)
            loop.quit()
            return False
        return True

    GLib.timeout_add(100, tick)
    try:
        loop.run()
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
    if errors:
        raise errors[0]
    print("PASS: close hides and holds; system Notify received while hidden; tray activation restores; menu Quit exits")
