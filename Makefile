SHELL := /bin/sh

.DEFAULT_GOAL := help
.PHONY: help test wasm-check shader-check build check serve dev clean

help: ## List available development commands.
	@awk 'BEGIN { FS = ":.*##"; printf "Usage: make <target>\n\nTargets:\n" } /^[a-zA-Z_-]+:.*##/ { printf "  %-14s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

test: ## Run focused Rust domain tests.
	@cargo test --locked --workspace

wasm-check: ## Check and lint the browser WebAssembly target.
	@cargo check --locked --target wasm32-unknown-unknown
	@cargo clippy --locked -p black-cube-gallery --target wasm32-unknown-unknown -- -D warnings

shader-check: ## Validate the active multipass and generated detailed shaders.
	@sh scripts/check_planet_shaders.sh

build: ## Build a static production site in dist/.
	@set -eu; \
		project_root=$$(pwd -P); \
		stage_parent="$$project_root/target"; \
		if [ -L "$$stage_parent" ]; then \
			printf '%s\n' "Refusing symlinked build staging directory: $$stage_parent" >&2; \
			exit 1; \
		fi; \
		mkdir -p "$$stage_parent"; \
		if [ ! -d "$$stage_parent" ] || [ -L "$$stage_parent" ]; then \
			printf '%s\n' "Build staging path is not a safe directory: $$stage_parent" >&2; \
			exit 1; \
		fi; \
		stage_root=$$(mktemp -d "$$stage_parent/gallery-publish.XXXXXX"); \
		case "$$stage_root" in \
			"$$stage_parent"/gallery-publish.*) ;; \
			*) rmdir -- "$$stage_root"; printf '%s\n' "Could not create a bounded build staging directory." >&2; exit 1 ;; \
		esac; \
		staged_site="$$stage_root/site"; \
		previous_site="$$stage_root/previous"; \
		dist="$$project_root/dist"; \
		published=0; \
		previous_moved=0; \
		cleanup() { \
			status=$$?; \
			trap - EXIT HUP INT TERM; \
			if [ "$$published" -eq 0 ] && [ "$$previous_moved" -eq 1 ] \
				&& [ ! -e "$$dist" ] && [ ! -L "$$dist" ]; then \
				mv -- "$$previous_site" "$$dist" || status=1; \
			fi; \
			rm -rf -- "$$stage_root"; \
			exit "$$status"; \
		}; \
		trap cleanup EXIT; \
		trap 'exit 129' HUP; \
		trap 'exit 130' INT; \
		trap 'exit 143' TERM; \
		mkdir "$$staged_site"; \
		wasm-pack build --target web --release --out-dir "$$staged_site/pkg" --out-name gallery; \
		cp "$$project_root/index.html" "$$project_root/styles.css" "$$project_root/bootstrap.js" "$$staged_site/"; \
		cp -R "$$project_root/assets" "$$staged_site/"; \
		if [ -L "$$dist" ] || { [ -e "$$dist" ] && [ ! -d "$$dist" ]; }; then \
			printf '%s\n' "Refusing to replace a non-directory or symlinked dist path: $$dist" >&2; \
			exit 1; \
		fi; \
		if [ -d "$$dist" ]; then \
			previous_moved=1; \
			mv -- "$$dist" "$$previous_site"; \
		fi; \
		mv -- "$$staged_site" "$$dist"; \
		published=1; \
		rm -rf -- "$$stage_root"; \
		trap - EXIT HUP INT TERM

check: test wasm-check shader-check ## Run formatting, lint, security, tooling, and production-build checks.
	@cargo fmt --check
	@cargo clippy --locked --workspace --all-targets -- -D warnings
	@PYTHONPYCACHEPREFIX=target/pycache python3 -m py_compile scripts/*.py
	@node --test scripts/*.test.mjs
	@output=$$(grep -RInE --exclude-dir=target 'innerHTML|outerHTML|insertAdjacentHTML|localStorage|sessionStorage|fetch\(|\.forget\(\)' index.html bootstrap.js src crates styles.css 2>&1); status=$$?; \
		if [ $$status -eq 0 ]; then printf '%s\n' "$$output"; exit 1; \
		elif [ $$status -gt 1 ]; then printf '%s\n' "$$output" >&2; exit $$status; fi
	@$(MAKE) --no-print-directory build

serve: build ## Serve the compiled gallery at http://127.0.0.1:8080/.
	@node scripts/serve.mjs

dev: build ## Watch source changes, rebuild Wasm, and refresh local browser tabs.
	@node scripts/serve.mjs --watch

clean: ## Remove generated local build artifacts.
	@rm -rf -- dist target crates/gallery-core/target crates/artwork-black-cube/target crates/artwork-world-in-light/target
