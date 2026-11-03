# make builds; make install symlinks the binary and stanley-popup into
# $(PREFIX)/bin, so a rebuild or an edited popup goes live without reinstalling.
PREFIX ?= $(HOME)/.local

all:
	cargo build --release --quiet

install: all
	mkdir -p "$(DESTDIR)$(PREFIX)/bin"
	ln -sfn "$(CURDIR)/target/release/stanley" "$(DESTDIR)$(PREFIX)/bin/stanley"
	ln -sfn "$(CURDIR)/src/popups/stanley-popup" "$(DESTDIR)$(PREFIX)/bin/stanley-popup"

uninstall:
	rm -f "$(DESTDIR)$(PREFIX)/bin/stanley" "$(DESTDIR)$(PREFIX)/bin/stanley-popup"

clean:
	cargo clean

.PHONY: all install uninstall clean
