#!/usr/bin/env python3
"""Run against a live Mango session. Changes and restores the current layout."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi

BINARY = Path(sys.argv[1] if len(sys.argv) > 1 else "target/release/mango-layout-tray").resolve()
APP_ID = "io.github.mikkelrask.MangoLayoutTray"
if subprocess.run(["busctl", "--user", "status", APP_ID], capture_output=True).returncode == 0:
    raise SystemExit("Quit the existing tray instance before running this live test.")


def command(*args):
    return subprocess.check_output(args, text=True)


def monitors():
    return json.loads(command("mmsg", "get", "all-monitors"))["monitors"]


def walk(node):
    yield node
    for i in range(node.get_child_count()):
        yield from walk(node.get_child_at_index(i))


def find(name=None, role=None):
    for node in walk(Atspi.get_desktop(0)):
        if node.get_role_name() == "application" and node.get_name() == "mango-layout-tray":
            for child in walk(node):
                if (name is None or child.get_name() == name) and (role is None or child.get_role_name() == role):
                    return child
    return None


def wait_for(predicate, description):
    deadline = time.monotonic() + 6
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.1)
    raise AssertionError(description)


def click(name, role="button"):
    node = wait_for(lambda: find(name, role), f"Missing {name}")
    assert node.get_action_iface().do_action(0), name


def search(text):
    entry = wait_for(lambda: find(role="entry"), "Missing search")
    assert entry.get_editable_text_iface().set_text_contents(text)
    time.sleep(0.25)


def screenshot(name):
    if not os.environ.get("MANGO_SMOKE_SCREENSHOTS"):
        return
    title = find("Find your flow.", "label")
    panel = title.get_parent().get_parent().get_parent()
    rect = panel.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
    output = Path(os.environ["MANGO_SMOKE_SCREENSHOTS"])
    output.mkdir(parents=True, exist_ok=True)
    # GTK cannot report global Wayland positions. Use our documented anchors.
    monitor = next(m for m in monitors() if m["active"])
    x = monitor["x"] + monitor["width"] - 16 - rect.width
    y = monitor["y"] + (12 if name == "drawer.png" else 24)
    command("grim", "-g", f"{x},{y} {rect.width}x{rect.height}", str(output / name))


original = next(m for m in monitors() if m["active"])
layouts = json.loads(command("mmsg", "get", "layouts"))["layouts"]
original_layout = next(l["name"] for l in layouts if l["symbol"] == original["layout_symbol"])
with tempfile.TemporaryDirectory(prefix="mango-tray-smoke-") as directory:
    env = dict(os.environ, XDG_CONFIG_HOME=directory)
    config_path = Path(directory) / "mango-layout-tray/config.toml"
    config_path.parent.mkdir()
    config_path.write_text('theme = "dark"\n')
    log_path = Path(directory) / "app.log"
    with log_path.open("w") as log:
        process = subprocess.Popen([str(BINARY), "--show"], env=env, stdout=log, stderr=log)
        try:
            wait_for(lambda: find("Find your flow.", "label"), "Picker did not open")
            screenshot("compact.png")
            for layout in layouts:
                subprocess.run([str(BINARY), "--show"], env=env, check=True)
                search(layout["name"].replace("_", " "))
                title = " ".join([layout["name"].split("_")[0].capitalize(), *layout["name"].split("_")[1:]])
                click(title)
                wait_for(lambda: next(m for m in monitors() if m["name"] == original["name"])["layout_symbol"] == layout["symbol"], f"Selection failed: {layout['name']}")
                services = command("busctl", "--user", "list")
                service = next(line.split()[0] for line in services.splitlines() if f"StatusNotifierItem-{process.pid}-" in line)
                wait_for(lambda: f"[{layout['symbol'].lower()}]" in command("busctl", "--user", "get-property", service, "/StatusNotifierItem", "org.kde.StatusNotifierItem", "Title"), "Tray did not update")
            command("mmsg", "dispatch", f"setlayout,{original_layout}")
            wait_for(lambda: f"[{original['layout_symbol'].lower()}]" in command("busctl", "--user", "get-property", service, "/StatusNotifierItem", "org.kde.StatusNotifierItem", "Title"), "External layout change did not reach tray")
            subprocess.run([str(BINARY), "--show"], env=env, check=True)
            search("zzzz")
            wait_for(lambda: find("No matching layouts. Try another name.", "label"), "Empty search state missing")
            search("")
            click("★", "toggle button")
            wait_for(lambda: 'favorites = ["tile"]' in config_path.read_text(), "Favorite did not persist")
            click("Settings")
            wait_for(lambda: find("Make it yours.", "label"), "Settings did not open")
            switch = find(role="switch")
            assert switch.get_action_iface().do_action(0)
            wait_for(lambda: "drawer = true" in config_path.read_text(), "Drawer setting did not persist")
            click("Done")
            subprocess.run([str(BINARY), "--show"], env=env, check=True)
            wait_for(lambda: find("Find your flow.", "label"), "Drawer did not open")
            time.sleep(0.3)
            screenshot("drawer.png")
            if os.environ.get("WTYPE"):
                wtype = os.environ["WTYPE"]
                subprocess.run([wtype, "-k", "Escape"], check=True)
                subprocess.run([str(BINARY), "--show"], env=env, check=True)
                wait_for(lambda: find("Find your flow.", "label"), "Keyboard picker did not open")
                subprocess.run([wtype, "dwindle"], check=True)
                time.sleep(0.2)
                subprocess.run([wtype, "-k", "Down", "-k", "Return"], check=True)
                wait_for(lambda: next(m for m in monitors() if m["name"] == original["name"])["layout_symbol"] == "DW", "Keyboard selection failed")
                subprocess.run([str(BINARY), "--show"], env=env, check=True)
                wait_for(lambda: find("Find your flow.", "label"), "Picker did not reopen")
                subprocess.run([wtype, "tile", "-k", "Return"], check=True)
                wait_for(lambda: next(m for m in monitors() if m["name"] == original["name"])["layout_symbol"] == "T", "Search Enter failed")
                command("mmsg", "dispatch", f"setlayout,{original_layout}")
                subprocess.run([str(BINARY), "--show"], env=env, check=True)
                wait_for(lambda: find("Find your flow.", "label"), "Picker did not reopen")
            click("Close (Escape)")
            # Standard StatusNotifier activation must reopen the same instance.
            command("busctl", "--user", "call", service, "/StatusNotifierItem", "org.kde.StatusNotifierItem", "Activate", "ii", "0", "0")
            wait_for(lambda: find("Find your flow.", "label"), "Tray activation failed")
            print("PASS: all 14 layouts, tray updates, external IPC, search, favorites, drawer, and single-instance activation")
        finally:
            command("mmsg", "dispatch", f"setlayout,{original_layout}")
            subprocess.run([str(BINARY), "--quit"], env=env, check=False)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=5)
            errors = log_path.read_text()
            if "CRITICAL" in errors or "panicked" in errors:
                print(errors, file=sys.stderr)
                raise AssertionError("Application emitted a critical error")
