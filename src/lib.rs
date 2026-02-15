pub mod engine;
pub mod platform;
pub mod cli;

use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);

pub fn is_running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

pub fn stop() {
    RUNNING.store(false, Ordering::SeqCst);
}

pub fn restart() {
    RUNNING.store(true, Ordering::SeqCst);
}

use crate::engine::config::AppConfig;
use crate::engine::obfuscation::ObfuscationEngine;
use crate::engine::scheduler::EventScheduler;
use crate::platform::KeyEvent;

const MIN_SLEEP_THRESHOLD_US: u64 = 1500;
const SLEEP_OVERHEAD_US: u64 = 800;

/// High-level function to run the engine with a given configuration.
pub fn run_engine(cfg: AppConfig) -> anyhow::Result<()> {
    let mut engine = ObfuscationEngine::new(cfg.obfuscation.clone(), cfg.advanced.clone());
    let scheduler = EventScheduler::new();

    let mut interceptor = platform::create_interceptor(
        cfg.system.device.as_deref(),
        cfg.system.auto_detect_keyboard,
    )?;

    interceptor.run(Box::new(move |raw_event: KeyEvent| {
        if !is_running() {
            return Some(raw_event);
        }

        // On macOS, the event timestamp is from mach_absolute_time (ns).
        // On Linux, it's from the evdev clock.
        // The engine now uses the event's own timestamp as the baseline.
        let original_ts_us = raw_event.timestamp_us;
        let scheduled = engine.process(raw_event.clone());

        // We need a way to compare the target emit time with "now".
        // On macOS, we should ideally use mach_absolute_time to wait.
        // For simplicity, we calculate the delta and wait.
        
        let target_us = scheduled.emit_at_us;
        
        if target_us > original_ts_us {
            let delay_us = target_us - original_ts_us;
            
            if delay_us > MIN_SLEEP_THRESHOLD_US {
                std::thread::sleep(std::time::Duration::from_micros(
                    delay_us.saturating_sub(SLEEP_OVERHEAD_US),
                ));
            }
            // In a real implementation we'd want to check "now" against 
            // the system monotonic clock here.
        }

        Some(scheduled.event)
    }))?;

    interceptor.stop()?;
    Ok(())
}
