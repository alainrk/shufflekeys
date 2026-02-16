use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Full application configuration, loaded from TOML or CLI overrides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub obfuscation: ObfuscationConfig,
    #[serde(default)]
    pub advanced: AdvancedConfig,
    #[serde(default)]
    pub system: SystemConfig,
}

/// Core obfuscation parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObfuscationConfig {
    /// Obfuscation strength: 0.0 = passthrough, 1.0 = full obfuscation.
    #[serde(default = "default_strength")]
    pub strength: f64,

    /// Maximum latency (ms) the engine may add to any single event.
    #[serde(default = "default_max_latency_ms")]
    pub max_latency_ms: f64,

    /// Quantisation bucket for dwell times (ms).
    #[serde(default = "default_dwell_bucket_ms")]
    pub dwell_bucket_ms: f64,

    /// Quantisation bucket for flight times (ms).
    #[serde(default = "default_flight_bucket_ms")]
    pub flight_bucket_ms: f64,

    /// Standard deviation for the Gaussian noise added after quantisation (ms).
    #[serde(default = "default_noise_stddev_ms")]
    pub noise_stddev_ms: f64,
}

/// Advanced timing constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    #[serde(default = "default_min_dwell_ms")]
    pub min_dwell_ms: f64,
    #[serde(default = "default_max_dwell_ms")]
    pub max_dwell_ms: f64,
    #[serde(default = "default_min_flight_ms")]
    pub min_flight_ms: f64,
    #[serde(default = "default_max_flight_ms")]
    pub max_flight_ms: f64,
    /// Generate a fresh persona seed every time the daemon starts.
    #[serde(default = "default_true")]
    pub new_persona_on_restart: bool,
}

/// Platform-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Linux: automatically find the keyboard input device.
    #[serde(default = "default_true")]
    pub auto_detect_keyboard: bool,
    /// Linux: explicit device path override (e.g. `/dev/input/event3`).
    pub device: Option<String>,
}

// ── defaults ────────────────────────────────────────────────────────────────

fn default_strength() -> f64 {
    0.7
}
fn default_max_latency_ms() -> f64 {
    25.0
}
fn default_dwell_bucket_ms() -> f64 {
    15.0
}
fn default_flight_bucket_ms() -> f64 {
    20.0
}
fn default_noise_stddev_ms() -> f64 {
    8.0
}
fn default_min_dwell_ms() -> f64 {
    40.0
}
fn default_max_dwell_ms() -> f64 {
    200.0
}
fn default_min_flight_ms() -> f64 {
    20.0
}
fn default_max_flight_ms() -> f64 {
    400.0
}
fn default_true() -> bool {
    true
}

// ── trait impls ─────────────────────────────────────────────────────────────

impl Default for ObfuscationConfig {
    fn default() -> Self {
        Self {
            strength: default_strength(),
            max_latency_ms: default_max_latency_ms(),
            dwell_bucket_ms: default_dwell_bucket_ms(),
            flight_bucket_ms: default_flight_bucket_ms(),
            noise_stddev_ms: default_noise_stddev_ms(),
        }
    }
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            min_dwell_ms: default_min_dwell_ms(),
            max_dwell_ms: default_max_dwell_ms(),
            min_flight_ms: default_min_flight_ms(),
            max_flight_ms: default_max_flight_ms(),
            new_persona_on_restart: true,
        }
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            auto_detect_keyboard: true,
            device: None,
        }
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

impl AppConfig {
    /// Resolve the config file path (~/.config/shufflekeys/config.toml).
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("shufflekeys")
            .join("config.toml")
    }

    /// Clamp all config values to valid ranges.
    pub fn validate(&mut self) {
        let obf = &mut self.obfuscation;
        obf.strength = obf.strength.clamp(0.0, 1.0);
        obf.max_latency_ms = obf.max_latency_ms.max(0.0);
        obf.dwell_bucket_ms = obf.dwell_bucket_ms.max(1.0);
        obf.flight_bucket_ms = obf.flight_bucket_ms.max(1.0);
        obf.noise_stddev_ms = obf.noise_stddev_ms.max(0.0);

        let adv = &mut self.advanced;
        adv.min_dwell_ms = adv.min_dwell_ms.max(0.0);
        adv.max_dwell_ms = adv.max_dwell_ms.max(adv.min_dwell_ms);
        adv.min_flight_ms = adv.min_flight_ms.max(0.0);
        adv.max_flight_ms = adv.max_flight_ms.max(adv.min_flight_ms);
    }

    /// Load from disk, falling back to built-in defaults when the file
    /// doesn't exist.
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::default_path();
        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let mut cfg: AppConfig = toml::from_str(&text)?;
            cfg.validate();
            log::info!("Loaded config from {}", path.display());
            Ok(cfg)
        } else {
            log::info!("No config file at {}; using defaults", path.display());
            Ok(AppConfig::default())
        }
    }

    /// Write the current config to disk (creates parent dirs).
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::default_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        log::info!("Saved config to {}", path.display());
        Ok(())
    }
}
