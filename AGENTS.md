# Mango Layout Tray

Rust tray utility for MangoWM. Use GTK4 for native theme colors, gtk4-layer-shell
for the compact picker and edge drawer, and ksni for StatusNotifierItem support.
GPUI was evaluated first; its Wayland backend lacks layer-shell surfaces.

## Product decisions

- Capture the invoked monitor when opening; apply to its active tags only.
- Track external layout changes using Mango's IPC event stream.
- Offer compact and drawer modes, favorites, layout ordering, and opt-in autostart.
- Follow GTK colors by default; allow light and dark overrides.
- Keep idle work low. No repeated subprocess polling.
- Use wireframe previews for all 14 documented layouts.
- Do not tie tray behavior to a particular panel.

## Working here

Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test`. Verify UI changes on a running Wayland/Mango session. Restore any
layout changed during testing. Keep comments sparse and commits small.
Update README when behavior or installation changes. Apply the humanizer skill
when available; write plain, specific prose either way.

Commits may be pushed to origin. CI checks main and pull requests. Version tags
(`v*`) build release archives and distro packages and publish GitHub releases.
Never commit local settings, secrets, build output, or temporary screenshots.

`idea.md` is the original user-owned brief; do not overwrite it. The leftover
`package.json` is user-owned scaffolding and is not used by this Rust app.
