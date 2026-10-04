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
`scripts/live-smoke.py` uses AT-SPI against a real session and restores the
original layout. Quit any existing app instance first. Optional `WTYPE` enables
keyboard checks. Keep screenshot captures limited to the app's own panel.
Update README when behavior or installation changes. Apply the humanizer skill
when available; write plain, specific prose either way.

Commits may be pushed to origin. CI checks main and pull requests. Version tags
(`v*`) build release archives and distro packages and publish GitHub releases.
After pushing each new version tag, wait for the GitHub Actions release build to
finish successfully and confirm its release assets are published. Then update
the sibling AUR repository `../mango-layout-tray-bin`: bump `pkgver`, reset
`pkgrel` to `1`, and pin the SHA256 checksum of the published GitHub archive.
Regenerate `.SRCINFO` with `makepkg --printsrcinfo > .SRCINFO`, verify the package
with `makepkg --cleanbuild --noconfirm`, then commit and push to its AUR remote.
Verify the remote commit matches the local commit. A release is not complete
until the AUR package is updated; report any publishing failure explicitly.

Never commit local settings, secrets, build output, or temporary screenshots.

`idea.md` is the original user-owned brief; do not overwrite it. The leftover
`package.json` is user-owned scaffolding and is not used by this Rust app.
