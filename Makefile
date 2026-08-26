# shclap Makefile

BINARY_NAME := shclap
CARGO := cargo
INSTALL_PATH := /usr/local/bin
MUSL_TARGET := x86_64-unknown-linux-musl
RUST_VERSION := 1.85.0

.PHONY: help setup-build-env build release test unit-test integration-test fmt fmt-check lint check install uninstall clean deb install-deb uninstall-deb coverage tag-release

.DEFAULT_GOAL := help

## help - Display this help message
help:
	@echo "Available targets:"
	@grep -E '^## [a-zA-Z]' $(MAKEFILE_LIST) | sed 's/## //' | awk -F ' - ' '{printf "  %-18s %s\n", $$1, $$2}'

## setup-build-env - Install Rust, musl target, cargo-deb, cargo-tarpaulin, and musl-tools
setup-build-env:
	@echo "Installing Rust $(RUST_VERSION)..."
	rustup install $(RUST_VERSION)
	rustup default $(RUST_VERSION)
	@echo "Adding musl target..."
	rustup target add $(MUSL_TARGET)
	@echo "Installing musl-tools..."
	sudo apt-get update && sudo apt-get install -y musl-tools
	@echo "Installing cargo-deb..."
	$(CARGO) install cargo-deb
	@echo "Installing cargo-tarpaulin..."
	$(CARGO) install cargo-tarpaulin@0.31.0
	@echo "Build environment setup complete."

## build - Build the project in debug mode
build:
	$(CARGO) build

## release - Build static release binary with musl
release:
	$(CARGO) build --release --target $(MUSL_TARGET)

## tag-release - Tag and push a release (requires VERSION=x.y.z); safe to re-run
tag-release:
	@if [ -z "$(VERSION)" ]; then echo "ERROR: VERSION not set (e.g., make tag-release VERSION=0.3.2)"; exit 1; fi
	@branch=$$(git rev-parse --abbrev-ref HEAD); \
	if [ "$$branch" != "main" ]; then \
		echo "ERROR: releases are cut from main (currently on '$$branch')"; exit 1; \
	fi
	@bad=$$(git diff -U0 -- Cargo.toml Cargo.lock | grep '^+[^+]' | grep -vx '+version = "$(VERSION)"' || true); \
	bad="$$bad$$(git diff -U0 -- Cargo.toml Cargo.lock | grep '^-[^-]' | grep -v '^-version = ' || true)"; \
	if [ -n "$$bad" ]; then \
		echo "ERROR: Cargo.toml or Cargo.lock has uncommitted changes beyond the version bump:"; \
		echo "$$bad"; exit 1; \
	fi
	sed -i 's/^version = "[^"]*"/version = "$(VERSION)"/' Cargo.toml
	$(CARGO) update -p $(BINARY_NAME)
	git add Cargo.toml Cargo.lock
	@pending=0; git diff --cached --quiet -- Cargo.toml Cargo.lock || pending=1; \
	head=$$(git rev-parse HEAD); \
	local_tag=$$(git rev-parse -q --verify "refs/tags/v$(VERSION)^{commit}" || true); \
	remote_tag=$$(git ls-remote origin "refs/tags/v$(VERSION)" "refs/tags/v$(VERSION)^{}" | tail -1 | awk '{print $$1}'); \
	for existing in $$local_tag $$remote_tag; do \
		if [ "$$pending" = "1" ] || [ "$$existing" != "$$head" ]; then \
			echo "ERROR: tag v$(VERSION) already exists at $$existing (HEAD is $$head)"; \
			echo "       pick a new VERSION, or delete the tag to re-cut the release"; \
			exit 1; \
		fi; \
	done; \
	if [ "$$pending" = "1" ]; then \
		git commit -m "chore(release): bump version to $(VERSION)"; \
	else \
		echo "Version already committed as $(VERSION), reusing HEAD"; \
	fi; \
	if [ -z "$$local_tag" ]; then \
		git tag v$(VERSION); \
	else \
		echo "Tag v$(VERSION) already points at HEAD, reusing"; \
	fi
	git push origin main
	git push origin v$(VERSION)
	@echo "Released v$(VERSION)"

## test - Run all tests (unit + integration)
test: unit-test integration-test

## unit-test - Run Rust unit tests
unit-test:
	$(CARGO) test

## integration-test - Run shell integration tests
integration-test: build
	./tests/integration.sh

## fmt - Format code with rustfmt
fmt:
	$(CARGO) fmt

## fmt-check - Check formatting without modifying files
fmt-check:
	$(CARGO) fmt --check

## lint - Run clippy linter
lint:
	$(CARGO) clippy -- -D warnings

## check - Run fmt, lint, and test
check: fmt lint test

## install - Install binary to system ($(INSTALL_PATH))
install: release
	sudo cp target/$(MUSL_TARGET)/release/$(BINARY_NAME) $(INSTALL_PATH)/$(BINARY_NAME)
	@echo "Installed $(BINARY_NAME) to $(INSTALL_PATH)"

## uninstall - Remove binary from system
uninstall:
	sudo rm -f $(INSTALL_PATH)/$(BINARY_NAME)
	@echo "Uninstalled $(BINARY_NAME) from $(INSTALL_PATH)"

## clean - Clean build artifacts
clean:
	$(CARGO) clean
	rm -rf target/
	rm -f *.deb

## deb - Build Debian package
deb: release
	$(CARGO) deb --target $(MUSL_TARGET)

## install-deb - Install the Debian package
install-deb:
	sudo dpkg -i target/$(MUSL_TARGET)/debian/*.deb

## uninstall-deb - Remove the installed Debian package
uninstall-deb:
	sudo dpkg -r $(BINARY_NAME)

## coverage - Generate test coverage report
coverage:
	$(CARGO) tarpaulin --out Html --out Stdout --output-dir coverage/
	@echo "Coverage report generated in coverage/tarpaulin-report.html"
