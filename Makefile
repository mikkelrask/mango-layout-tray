PREFIX ?= /usr/local
DESTDIR ?=

.PHONY: build check install uninstall
build:
	cargo build --release --locked
check:
	cargo fmt --check
	cargo clippy --locked --all-targets -- -D warnings
	cargo test --locked
install: build
	install -Dm755 target/release/mango-layout-tray $(DESTDIR)$(PREFIX)/bin/mango-layout-tray
	install -Dm644 data/mango-layout-tray.desktop $(DESTDIR)$(PREFIX)/share/applications/mango-layout-tray.desktop
	install -Dm644 data/mango-layout-tray.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/mango-layout-tray.svg
	install -Dm644 LICENSE $(DESTDIR)$(PREFIX)/share/licenses/mango-layout-tray/LICENSE
uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/mango-layout-tray $(DESTDIR)$(PREFIX)/share/applications/mango-layout-tray.desktop $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/mango-layout-tray.svg $(DESTDIR)$(PREFIX)/share/licenses/mango-layout-tray/LICENSE
