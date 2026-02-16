# ShuffleKeys — Developer Makefile
# ============================================================================
#
# Core library (Rust):     src/          → binary "shufflekeys"
# Desktop app:             src-tauri/    → desktop app wrapping the library
# Frontend (React/Vite):   src/*.tsx      → bundled into dist/
#
# Usage:
#   make              — lint, test, build (debug)
#   make app          — launch the desktop app (dev mode)
#   make release      — full release build (CLI + desktop app)
#   make check        — fast compile check without codegen
#   make lint         — clippy + fmt check
#   make test         — run all tests
#   make clean        — remove all build artifacts

.PHONY: all check build release test lint clippy fmt fmt-check clean \
        install uninstall setup init-config \
        app app-build app-clean \
        frontend-install frontend-dev frontend-build frontend-clean \
        run run-off run-status run-daemon \
        ci help

# ── Configuration ───────────────────────────────────────────────────────────

CARGO       := cargo
NPM         := npm
TAURI       := cargo tauri
BIN_NAME    := shufflekeys
INSTALL_DIR := /usr/local/bin
CONFIG_DIR  := $(HOME)/.config/shufflekeys

# ── Default target ──────────────────────────────────────────────────────────

all: lint test build

# ── Core Rust library + CLI ─────────────────────────────────────────────────

## Fast compile check (no codegen, catches errors quickly)
check:
	$(CARGO) check --all-targets

## Debug build
build:
	$(CARGO) build

## Optimised release build
release:
	$(CARGO) build --release

## Run all tests (unit + doc)
test:
	$(CARGO) test

## Run tests and show output (including passing tests)
test-verbose:
	$(CARGO) test -- --nocapture

# ── Linting & Formatting ───────────────────────────────────────────────────

## Run all lint checks (clippy + fmt)
lint: clippy fmt-check

## Run clippy with project-configured lints
clippy:
	$(CARGO) clippy --all-targets -- -D warnings

## Auto-format all Rust files
fmt:
	$(CARGO) fmt

## Check formatting without modifying files
fmt-check:
	$(CARGO) fmt -- --check

# ── Frontend (React / Vite / Tailwind) ──────────────────────────────────────

## Install frontend npm dependencies
frontend-install:
	$(NPM) install

## Start frontend dev server only (no desktop app)
frontend-dev: frontend-install
	$(NPM) run dev

## Production build of the frontend (outputs to dist/)
frontend-build: frontend-install
	$(NPM) run build

## Remove frontend build artifacts
frontend-clean:
	rm -rf dist node_modules

# ── Desktop App ───────────────────────────────────────────────────────────

## Launch the desktop app in dev mode (hot-reload)
app: frontend-install
	$(TAURI) dev

## Build the desktop app for distribution
app-build: frontend-install
	$(TAURI) build

## Remove desktop app build artifacts
app-clean:
	rm -rf src-tauri/target

# ── Run (CLI) ──────────────────────────────────────────────────────────────

## Run the CLI in debug mode (obfuscation ON)
run: build
	sudo $(CARGO) run

## Run with obfuscation disabled (passthrough mode)
run-off: build
	sudo $(CARGO) run -- off

## Show current status and config
run-status: build
	$(CARGO) run -- status

## Run as a background daemon
run-daemon: release
	sudo ./target/release/$(BIN_NAME) --daemon

# ── Install / Uninstall ────────────────────────────────────────────────────

## Install release binary to /usr/local/bin (requires sudo)
install: release
	sudo cp ./target/release/$(BIN_NAME) $(INSTALL_DIR)/$(BIN_NAME)
	@echo "Installed $(BIN_NAME) to $(INSTALL_DIR)"

## Remove installed binary
uninstall:
	sudo rm -f $(INSTALL_DIR)/$(BIN_NAME)
	@echo "Removed $(BIN_NAME) from $(INSTALL_DIR)"

## Write default config to ~/.config/shufflekeys/config.toml
init-config: build
	$(CARGO) run -- init-config

## Run the interactive setup wizard (build + permissions + service)
setup: release
	./setup.sh

# ── Clean ──────────────────────────────────────────────────────────────────

## Remove all Rust build artifacts
clean:
	$(CARGO) clean

## Remove everything (Rust + frontend + desktop app)
clean-all: clean frontend-clean app-clean

# ── CI (all checks a PR must pass) ─────────────────────────────────────────

## Full CI pipeline: fmt check, clippy, test, release build
ci: fmt-check clippy test release
	@echo "CI passed."

# ── Help ───────────────────────────────────────────────────────────────────

## Show this help
help:
	@echo "ShuffleKeys — Developer Makefile"
	@echo ""
	@echo "Usage: make <target>"
	@echo ""
	@echo "Core:"
	@echo "  check          Fast compile check (no codegen)"
	@echo "  build          Debug build"
	@echo "  release        Optimised release build"
	@echo "  test           Run all tests"
	@echo "  test-verbose   Run tests with output"
	@echo ""
	@echo "Linting:"
	@echo "  lint           Run clippy + fmt check"
	@echo "  clippy         Run clippy"
	@echo "  fmt            Auto-format Rust code"
	@echo "  fmt-check      Check formatting (no changes)"
	@echo ""
	@echo "Desktop App:"
	@echo "  app            Launch desktop app (dev mode, hot-reload)"
	@echo "  app-build      Build desktop app for distribution"
	@echo "  app-clean      Remove desktop app build artifacts"
	@echo ""
	@echo "Frontend only:"
	@echo "  frontend-dev   Start frontend dev server (no desktop app)"
	@echo "  frontend-build Production frontend build"
	@echo "  frontend-clean Remove dist/ and node_modules/"
	@echo ""
	@echo "Run (CLI):"
	@echo "  run            Run with obfuscation ON (sudo)"
	@echo "  run-off        Run in passthrough mode (sudo)"
	@echo "  run-status     Show status and config"
	@echo "  run-daemon     Run as background daemon (sudo)"
	@echo ""
	@echo "Install:"
	@echo "  install        Install release binary to $(INSTALL_DIR)"
	@echo "  uninstall      Remove installed binary"
	@echo "  init-config    Write default config file"
	@echo "  setup          Run interactive setup wizard"
	@echo ""
	@echo "Housekeeping:"
	@echo "  clean          Remove Rust build artifacts"
	@echo "  clean-all      Remove all build artifacts"
	@echo "  ci             Full CI pipeline (fmt + clippy + test + release)"
	@echo "  help           Show this help"
