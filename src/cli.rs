use clap::{Parser, Subcommand};

/// ShuffleKeys — Keystroke dynamics obfuscation tool.
///
/// Intercepts keyboard events at the OS level and injects controlled timing
/// noise to make typing-based biometric fingerprinting unreliable.
#[derive(Parser, Debug)]
#[command(name = "shufflekeys", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    // ── Global overrides (applied on top of config file) ───────────────

    /// Obfuscation strength (0.0 = passthrough, 1.0 = full).
    #[arg(long, value_name = "0.0-1.0")]
    pub strength: Option<f64>,

    /// Maximum added latency per event in ms.
    #[arg(long, value_name = "MS")]
    pub max_latency: Option<f64>,

    /// Quantisation bucket size in ms.
    #[arg(long, value_name = "MS")]
    pub bucket: Option<f64>,

    /// Gaussian noise standard deviation in ms.
    #[arg(long, value_name = "MS")]
    pub noise: Option<f64>,

    /// Linux: explicit input device path (e.g. /dev/input/event3).
    #[arg(long, value_name = "PATH")]
    pub device: Option<String>,

    /// Run as a background daemon (fork and detach).
    #[arg(long)]
    pub daemon: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Enable obfuscation (default).
    On,
    /// Disable obfuscation (passthrough mode).
    Off,
    /// Show current daemon status and config.
    Status,
    /// Generate a new persona seed (re-rolls all timing offsets).
    NewPersona,
    /// Write the default config file to ~/.config/shufflekeys/config.toml.
    InitConfig,
}
