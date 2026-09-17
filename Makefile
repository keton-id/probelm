SHELL := /bin/bash
CARGO ?= cargo

.PHONY: help fmt fmt-check lint check test build package npm-pack install uninstall check-all clean

help: ## Show available targets
	@awk 'BEGIN {FS = ":.*##"; print "Usage: make <target>\n"} /^[a-zA-Z0-9_-]+:.*##/ {printf "  %-14s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

fmt: ## Format Rust sources
	$(CARGO) fmt --all

fmt-check: ## Check Rust formatting
	$(CARGO) fmt --all -- --check

lint: ## Run Clippy with warnings denied
	$(CARGO) clippy --all-targets --all-features -- -D warnings

check: ## Type-check the locked dependency graph
	$(CARGO) check --all-targets --all-features --locked

test: ## Run the Rust test suite
	$(CARGO) test --all-targets --all-features --locked

build: ## Build release binaries
	$(CARGO) build --release --locked

package: ## Validate the crates.io package contents
	$(CARGO) package --locked --allow-dirty

npm-pack: ## Validate the npm package tarball
	cd npm && npm pack --dry-run


install: ## Install the current checkout with the Unix installer
	./script/install.sh

uninstall: ## Remove the Unix installation
	./script/install.sh --uninstall

check-all: fmt-check lint check test package npm-pack ## Run all local release checks

clean: ## Remove Cargo build output
	$(CARGO) clean
