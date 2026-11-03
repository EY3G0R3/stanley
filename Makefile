# make builds; make install symlinks the binary into $(PREFIX)/bin, so a
# rebuild goes live without reinstalling.
PREFIX ?= $(HOME)/.local

all:
	cargo build --release --quiet

install: all
	mkdir -p "$(DESTDIR)$(PREFIX)/bin"
	ln -sfn "$(CURDIR)/target/release/stanley" "$(DESTDIR)$(PREFIX)/bin/stanley"

uninstall:
	rm -f "$(DESTDIR)$(PREFIX)/bin/stanley"

clean:
	cargo clean

.PHONY: all install uninstall clean
