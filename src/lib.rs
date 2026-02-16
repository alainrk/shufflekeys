//! `ShuffleKeys` — Keystroke dynamics obfuscation library.
//!
//! Intercepts keyboard events at the OS level and applies controlled timing
//! noise to defeat typing-based biometric fingerprinting. The core engine
//! quantises, adds Gaussian jitter, and clamps keystroke timing to
//! physiologically plausible ranges, producing a per-session "persona" that
//! cannot be linked back to the real user.

pub mod cli;
pub mod engine;
pub mod platform;

use std::sync::atomic::{AtomicBool, Ordering};

use crate::engine::config::AppConfig;
use crate::engine::obfuscation::ObfuscationEngine;
use crate::platform::KeyEvent;

/// Minimum delay (µs) for which we use `thread::sleep`. Below this, we only busy-wait.
const MIN_SLEEP_THRESHOLD_US: u64 = 1500;
/// Overhead (µs) subtracted from sleep to avoid oversleeping past the target.
const SLEEP_OVERHEAD_US: u64 = 800;

static RUNNING: AtomicBool = AtomicBool::new(true);

/// Returns `true` if the engine should keep running.
pub fn is_running() -> bool {
    RUNNING.load(Ordering::Acquire)
}

/// Signal the engine to stop.
pub fn stop() {
    RUNNING.store(false, Ordering::Release);
}

/// Re-arm the running flag (e.g. after a stop/restart cycle).
pub fn restart() {
    RUNNING.store(true, Ordering::Release);
}

/// Run the obfuscation engine with the given configuration.
///
/// Blocks until the interceptor loop ends (via [`stop()`] or device disconnect).
pub fn run_engine(cfg: AppConfig, enabled: bool, new_persona: bool) -> anyhow::Result<()> {
    let mut engine = ObfuscationEngine::new(cfg.obfuscation, cfg.advanced);
    engine.set_enabled(enabled);
    if new_persona {
        engine.regenerate_persona();
    }

    let mut interceptor = platform::create_interceptor(
        cfg.system.device.as_deref(),
        cfg.system.auto_detect_keyboard,
    )?;

    interceptor.run(Box::new(move |raw_event: KeyEvent| {
        if !is_running() {
            return Some(raw_event);
        }

        let original_ts_us = raw_event.timestamp_us;
        let scheduled = engine.process(raw_event);

        let target_us = scheduled.emit_at_us;
        if target_us > original_ts_us {
            let delay_us = target_us - original_ts_us;
            let start = std::time::Instant::now();
            let target_duration = std::time::Duration::from_micros(delay_us);

            if delay_us > MIN_SLEEP_THRESHOLD_US {
                std::thread::sleep(std::time::Duration::from_micros(
                    delay_us.saturating_sub(SLEEP_OVERHEAD_US),
                ));
            }
            // Busy-wait for sub-ms precision.
            while start.elapsed() < target_duration {
                std::hint::spin_loop();
            }
        }

        Some(scheduled.event)
    }))?;

    interceptor.stop()?;
    Ok(())
}
