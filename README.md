# 🛡️ ShuffleKeys

**Defeat biometric fingerprinting by obfuscating your [keystroke dynamics](https://en.wikipedia.org/wiki/Keystroke_dynamics).**

[![Status: Active](https://img.shields.io/badge/Status-Active-success)](https://github.com/user/repo)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue)](LICENSE)

---

## The Problem

Websites use high-resolution JavaScript timers to capture your unique typing pattern—the timing between key presses (**flight time**) and the duration each key is held down (**dwell time**). This creates a biometric "fingerprint" that identifies you across sites, even when using a VPN or Incognito mode.

## The Solution

ShuffleKeys operates at the system level to neutralize tracking by intercepting physical keystrokes and re-emitting them as synthetic events.

### Core Mechanisms

- **Quantization:** Rounds timing to discrete "buckets" to eliminate precise millisecond patterns.
- **Controlled Noise:** Injects Gaussian noise ($\sigma$) into flight and dwell times.
- **Timestamp Spoofing:** Replaces raw hardware timestamps with spoofed data to mislead trackers.

---

## Dashboard & Configuration

<p align="center">
  <img src="assets/screen_1.png" alt="Main Dashboard" width="48%" />
  <img src="assets/screen_2.png" alt="Configuration Settings" width="48%" />
</p>

---

## Demo: Neutralizing Enrollment

In this demo, the biometric registration fails to verify the user because the underlying rhythm is replaced by a randomized "persona". [TypingDNA](https://www.typingdna.com/demo-sametext.html) provides a way to test it through their demo Authentication API.

<p align="center">
  <img src="assets/demo.gif" alt="ShuffleKeys Demo" width="600px" />
</p>

## Tech Stack

### Product

- **Core Engine**: [Rust](https://www.rust-lang.org/) — High-performance, low-latency keystroke manipulation using `nix`, `libc`, and OS-specific APIs (`evdev` on Linux, `CoreGraphics` on macOS).
- **Desktop Framework**: [Tauri v2](https://tauri.app/) — Lightweight desktop wrapper using native webviews for a minimal footprint.
- **Frontend UI**: [React](https://reactjs.org/) + [TypeScript](https://www.typescriptlang.org/) — Modern, type-safe interface for real-time monitoring and configuration.
- **Styling**: [Tailwind CSS](https://tailwindcss.com/) + [Lucide](https://lucide.dev/) — Sleek, utility-first design and iconography.

### Toolchain

- **Build System**: `Makefile` — Standardized entry points for CLI, UI, and maintenance tasks.
- **Backend Tooling**: `Cargo` (Package Manager), `Clippy` (Linter), `rustfmt` (Formatter).
- **Frontend Tooling**: `Vite` (Build Tool), `NPM` (Package Manager), `PostCSS` (CSS Transformation).

## Installation

### Prerequisites

- Rust toolchain (install via [rustup.rs](https://rustup.rs/))
- Node.js and npm

### Building from source

```bash
# Clone the repository
git clone https://github.com/your-repo/shufflekeys.git
cd shufflekeys

# Run the setup script to build and configure permissions
make setup
```

### macOS — Gatekeeper notice

ShuffleKeys is not signed with an Apple Developer certificate, so macOS will show a warning:

> "ShuffleKeys" cannot be opened because Apple could not verify it is free of malware.

To bypass this, remove the quarantine attribute after downloading:

```bash
xattr -cr /Applications/ShuffleKeys.app
```

Alternatively, right-click the app and select **Open** — macOS will give you the option to open it anyway.

## Usage

### Desktop App

The recommended way to use ShuffleKeys is through the desktop app:

```bash
make app
```

From the UI, you can toggle protection on and off and adjust the obfuscation strength.

### CLI

You can also run ShuffleKeys directly from the terminal:

```bash
make run            # Run with obfuscation ON (sudo)
make run-off        # Run in passthrough mode (sudo)
make run-status     # Show current config
```

### Developer Commands

```bash
make build          # Debug build
make test           # Run all tests
make lint           # Run clippy + format check
make fmt            # Auto-format code
make app-build      # Build desktop app for distribution
make ci             # Full CI pipeline
make help           # Show all available commands
```

### Permissions

#### macOS

The application requires **Accessibility** (and potentially **Input Monitoring**) permissions. The setup script will guide you to the correct menu in System Settings.

> **After reinstalling or updating**: macOS invalidates the existing accessibility grant when the binary changes. If the toggle is already on but ShuffleKeys reports missing permissions, remove the entry and re-add it in System Settings -> Privacy & Security -> Accessibility (select ShuffleKeys, click the **-** button, then re-open the app and add it back with **+**).

#### Linux

The application requires access to `/dev/uinput`. The setup script will create the necessary udev rules and add your user to the `input` group. You must log out and back in for these changes to take effect.

## Configuration

Settings can be tuned in the Desktop UI or manually in `~/.config/shufflekeys/config.toml`:

- **Strength**: Balance between original timing and full obfuscation (0.7 is recommended).
- **Max Latency**: The maximum delay (in ms) allowed for a single keystroke.
- **Buckets**: The quantization level for timing intervals.
- **Noise**: The amount of Gaussian jitter added to prevent bucket analysis.
