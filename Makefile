# make builds; make install symlinks the binary and stanley-popup into
# $(PREFIX)/bin, so a rebuild or an edited popup goes live without reinstalling.
PREFIX ?= $(HOME)/.local

all:
	cargo build --release --quiet

install: all
	mkdir -p "$(DESTDIR)$(PREFIX)/bin"
	ln -sfn "$(CURDIR)/target/release/stanley" "$(DESTDIR)$(PREFIX)/bin/stanley"
	ln -sfn "$(CURDIR)/src/popups/stanley-popup" "$(DESTDIR)$(PREFIX)/bin/stanley-popup"

test:
	cargo test --release --quiet

# For ~/bin/autoupdate-repos: autoupdate tests and builds a fetched commit in a
# scratch copy, then autoupdate-install rebuilds the live checkout and restarts
# the bar; dwl-session's loop starts the new build.
autoupdate: test all

autoupdate-install: install
	pkill -x stanley || true

uninstall:
	rm -f "$(DESTDIR)$(PREFIX)/bin/stanley" "$(DESTDIR)$(PREFIX)/bin/stanley-popup"

clean:
	cargo clean

.PHONY: all install test autoupdate autoupdate-install uninstall clean
