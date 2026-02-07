# shclap Makefile

BINARY_NAME := shclap
CARGO := cargo
INSTALL_PATH := /usr/local/bin
MUSL_TARGET := x86_64-unknown-linux-musl
RUST_VERSION := 1.81.0

.PHONY: help setup-build-env build release test unit-test integration-test fmt fmt-check lint check install uninstall clean deb install-deb uninstall-deb

.DEFAULT_GOAL := help

## help - Display this help message
help:
	@echo "Available targets:"
	@grep -E '^## [a-zA-Z]' $(MAKEFILE_LIST) | sed 's/## //' | awk -F ' - ' '{printf "  %-18s %s\n", $$1, $$2}'

## setup-build-env - Install Rust, musl target, cargo-deb, and musl-tools
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
	@echo "Build environment setup complete."

## build - Build the project in debug mode
build:
	$(CARGO) build

## release - Build static release binary with musl
release:
	$(CARGO) build --release --target $(MUSL_TARGET)

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
