#!/bin/bash
set -e

# Colors and Styling
BOLD=$(tput bold)
RESET=$(tput sgr0)
RED=$(tput setaf 1)
GREEN=$(tput setaf 2)
BLUE=$(tput setaf 4)
YELLOW=$(tput setaf 3)

APP_NAME="shufflekeys"
BIN_NAME="shufflekeys"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="$HOME/.config/shufflekeys"

header() {
  echo -e "
${BLUE}${BOLD}=== $1 ===${RESET}"
}

info() {
  echo -e "${BOLD}INFO:${RESET} $1"
}

success() {
  echo -e "${GREEN}${BOLD}SUCCESS:${RESET} $1"
}

warn() {
  echo -e "${YELLOW}${BOLD}WARNING:${RESET} $1"
}

error() {
  echo -e "${RED}${BOLD}ERROR:${RESET} $1"
}

ask_yes_no() {
  while true; do
    read -p "$1 [Y/n] " yn
    case $yn in
    [Yy]*) return 0 ;;
    [Nn]*) return 1 ;;
    "") return 0 ;;
    *) echo "Please answer yes or no." ;;
    esac
  done
}

# -----------------------------------------------------------------------------
# 1. Welcome
# -----------------------------------------------------------------------------
clear
echo -e "${BLUE}${BOLD}"
echo "   _____ _            __  __ _      __ __"
echo "  / ___/| |__  _   _ / _|/ _| | ___|  |  | ___ _   _ ___"
echo "  \___ \| '_ \| | | | |_| |_| |/ _ \ .-. |/ _ \ | | / __|"
echo "   ___) | | | | |_| |  _|  _| |  __/ | | |  __/ |_| \__ "
echo "  |____/|_| |_|\__,_|_| |_| |_|\___|_| |_|\___|\__, |___/"
echo "                                               |___/"
echo -e "${RESET}"
echo "Welcome to the ${BOLD}ShuffleKeys${RESET} Setup Wizard!"
echo "This script will help you build, install, and configure ShuffleKeys."
echo "OS Detected: $(uname -s)"
echo ""

# -----------------------------------------------------------------------------
# 2. Check Dependencies
# -----------------------------------------------------------------------------
header "Checking Prerequisites"

if ! command -v cargo &>/dev/null; then
  error "Rust/Cargo is not installed or not in PATH."
  echo "Please install Rust from https://rustup.rs/ and try again."
  exit 1
fi
success "Cargo found."

# -----------------------------------------------------------------------------
# 3. Build Release
# -----------------------------------------------------------------------------
header "Building Project"
info "Compiling release binary... (this might take a minute)"

if cargo build --release; then
  success "Build complete."
else
  error "Build failed. Please check the error messages above."
  exit 1
fi

# -----------------------------------------------------------------------------
# 4. Install Binary
# -----------------------------------------------------------------------------
header "Installation"

TARGET_BIN="./target/release/$BIN_NAME"

if ask_yes_no "Do you want to install '$BIN_NAME' to $INSTALL_DIR?"; then
  info "Installing binary (requires sudo)..."
  if sudo cp "$TARGET_BIN" "$INSTALL_DIR/$BIN_NAME"; then
    success "Installed to $INSTALL_DIR/$BIN_NAME"
  else
    error "Failed to install binary."
    exit 1
  fi
else
  info "Skipping installation. You can run it from ./target/release/$BIN_NAME"
  INSTALL_DIR=$(pwd)/target/release
fi

# -----------------------------------------------------------------------------
# 5. Initial Configuration
# -----------------------------------------------------------------------------
header "Configuration"

if [ ! -f "$CONFIG_DIR/config.toml" ]; then
  if ask_yes_no "Initialize default configuration at $CONFIG_DIR?"; then
    mkdir -p "$CONFIG_DIR"
    "$INSTALL_DIR/$BIN_NAME" init-config
    success "Configuration created."
  fi
else
  info "Configuration already exists at $CONFIG_DIR/config.toml"
fi

# -----------------------------------------------------------------------------
# 6. Platform Specific Setup
# -----------------------------------------------------------------------------
OS="$(uname -s)"
header "Platform Setup: $OS"

if [ "$OS" == "Linux" ]; then
  info "Linux setup: Checking permissions for /dev/uinput"

  # Check if uinput group exists (unlikely standard, usually 'input' group owns it)
  # Common rule: KERNEL=="uinput", GROUP="input", MODE="0660"

  if ask_yes_no "Configure udev rules for passwordless usage (recommended)?"; then
    RULE_FILE="/etc/udev/rules.d/99-shufflekeys.rules"
    RULE_CONTENT='KERNEL=="uinput", GROUP="input", MODE="0660"'

    info "Creating $RULE_FILE..."
    echo "$RULE_CONTENT" | sudo tee "$RULE_FILE" >/dev/null

    info "Adding user '$USER' to 'input' group..."
    sudo usermod -aG input "$USER"

    info "Reloading udev rules..."
    sudo udevadm control --reload-rules && sudo udevadm trigger

    success "Permissions configured."
    warn "You may need to LOG OUT and LOG BACK IN for group changes to apply."
  fi

elif [ "$OS" == "Darwin" ]; then
  info "macOS setup: Accessibility Permissions"
  echo "ShuffleKeys requires 'Accessibility' (Input Monitoring) permissions to intercept keystrokes."
  echo ""
  echo "1. The app will try to run now to trigger the permission prompt."
  echo "2. Open System Settings -> Privacy & Security -> Accessibility."
  echo "3. Ensure your Terminal (or shufflekeys binary) is checked."
  echo ""

  if ask_yes_no "Open System Settings now?"; then
    open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
  fi
fi

# -----------------------------------------------------------------------------
# 7. Service Setup (Daemon)
# -----------------------------------------------------------------------------
header "Service Setup"

if ask_yes_no "Do you want ShuffleKeys to start automatically at login?"; then
  if [ "$OS" == "Linux" ]; then
    # Create systemd user service
    SERVICE_DIR="$HOME/.config/systemd/user"
    SERVICE_FILE="$SERVICE_DIR/shufflekeys.service"
    mkdir -p "$SERVICE_DIR"

    cat <<EOF >"$SERVICE_FILE"
[Unit]
Description=ShuffleKeys Keystroke Obfuscation
After=network.target

[Service]
ExecStart=$INSTALL_DIR/$BIN_NAME --daemon
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF
    info "Created $SERVICE_FILE"
    systemctl --user daemon-reload
    systemctl --user enable shufflekeys
    success "Service enabled. It will start on next login."
    if ask_yes_no "Start the service now?"; then
      systemctl --user start shufflekeys
      success "Service started."
    fi

  elif [ "$OS" == "Darwin" ]; then
    # Create launchd agent
    PLIST_DIR="$HOME/Library/LaunchAgents"
    PLIST_FILE="$PLIST_DIR/com.user.shufflekeys.plist"
    LOG_DIR="$HOME/Library/Logs/shufflekeys"
    mkdir -p "$PLIST_DIR"
    mkdir -p "$LOG_DIR"

    cat <<EOF >"$PLIST_FILE"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.user.shufflekeys</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/$BIN_NAME</string>
        <string>--daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOG_DIR/shufflekeys.out.log</string>
    <key>StandardErrorPath</key>
    <string>$LOG_DIR/shufflekeys.err.log</string>
</dict>
</plist>
EOF
    info "Created $PLIST_FILE"
    launchctl load "$PLIST_FILE" 2>/dev/null || true
    success "Launch agent loaded."
  fi
else
  info "Skipping service setup."
fi

# -----------------------------------------------------------------------------
# 8. Done
# -----------------------------------------------------------------------------
header "Setup Complete"
echo "ShuffleKeys is ready!"
echo "Usage: sudo $BIN_NAME [OPTIONS]"
echo "Check '$BIN_NAME --help' for more info."
echo ""
echo -e "${GREEN}${BOLD}Enjoy your privacy!${RESET}"
