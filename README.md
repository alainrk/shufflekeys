# ShuffleKeys

ShuffleKeys is a tool designed to protect your privacy by obfuscating your keystroke dynamics.

<p align="center">
  <img src="assets/demo.gif" alt="ShuffleKeys Demo" width="600px" />
</p>

## How it works

Every person has a unique typing pattern—the specific timing between key presses (flight time) and the duration each key is held down (dwell time). Websites and trackers use high-resolution JavaScript timers to capture these patterns, creating a biometric "fingerprint" that can identify you across different sites, even if you use a VPN or incognito mode.

ShuffleKeys operates at the system level to neutralize this tracking. It intercepts your physical keystrokes, applies controlled timing noise and quantization (rounding the timing to discrete "buckets"), and then re-emits them as synthetic events with spoofed hardware timestamps. To a tracker, your unique typing rhythm is replaced by a consistent, randomized "persona" that cannot be linked back to you.

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
./setup.sh
```

## Usage

### Desktop UI
The recommended way to use ShuffleKeys is through the graphical interface.
```bash
cargo tauri dev
```
From the UI, you can toggle protection on and off and adjust the obfuscation strength.

### Permissions

#### macOS
The application requires **Accessibility** (and potentially **Input Monitoring**) permissions. The setup script will guide you to the correct menu in System Settings.

#### Linux
The application requires access to `/dev/uinput`. The setup script will create the necessary udev rules and add your user to the `input` group. You must log out and back in for these changes to take effect.

## Configuration

Settings can be tuned in the Desktop UI or manually in `~/.config/shufflekeys/config.toml`:
- **Strength**: Balance between original timing and full obfuscation (0.7 is recommended).
- **Max Latency**: The maximum delay (in ms) allowed for a single keystroke.
- **Buckets**: The quantization level for timing intervals.
- **Noise**: The amount of Gaussian jitter added to prevent bucket analysis.
