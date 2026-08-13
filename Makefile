SHELL := /bin/sh

.DEFAULT_GOAL := help
.PHONY: help test build check serve dev clean

help: ## List available development commands.
	@awk 'BEGIN { FS = ":.*##"; printf "Usage: make <target>\n\nTargets:\n" } /^[a-zA-Z_-]+:.*##/ { printf "  %-10s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

test: ## Run focused Rust domain tests.
	@cargo test

build: ## Build a static production site in dist/.
	@mkdir -p dist
	@wasm-pack build --target web --release --out-dir dist/pkg --out-name gallery
	@cp index.html styles.css bootstrap.js dist/

check: test ## Run formatting, lint, security, and production-build checks.
	@cargo fmt --check
	@cargo clippy --all-targets -- -D warnings
	@! rg -n 'innerHTML|outerHTML|insertAdjacentHTML|localStorage|sessionStorage|fetch\(' index.html bootstrap.js src styles.css
	@$(MAKE) --no-print-directory build

serve: build ## Serve the compiled gallery at http://127.0.0.1:8080.
	@node scripts/serve.mjs

dev: build ## Watch source changes, rebuild Wasm, and refresh local browser tabs.
	@node scripts/serve.mjs --watch

clean: ## Remove generated local build artifacts.
	@rm -rf -- dist target
