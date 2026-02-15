use anyhow::Context;
use clap::Parser;

use shufflekeys::cli::{Cli, Commands};
use shufflekeys::engine::config::AppConfig;
use shufflekeys::engine::obfuscation::ObfuscationEngine;
use shufflekeys::engine::scheduler::EventScheduler;
use shufflekeys::platform::{self, KeyEvent};

/// Minimum delay for which we use thread::sleep. Below this, we only busy-wait.
const MIN_SLEEP_THRESHOLD_US: u64 = 1500;
/// Amount of time to subtract from sleep to ensure we don't oversleep.
const SLEEP_OVERHEAD_US: u64 = 800;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let cli = Cli::parse();

    // Handle sub-commands that don't need the event loop.
    match &cli.command {
        Some(Commands::InitConfig) => {
            let cfg = AppConfig::default();
            cfg.save()?;
            println!(
                "Default config written to {}",
                AppConfig::default_path().display()
            );
            return Ok(());
        }
        Some(Commands::Status) => {
            println!("ShuffleKeys status");
            let cfg = AppConfig::load()?;
            println!("  Config file: {}", AppConfig::default_path().display());
            println!("  Strength:    {:.1}", cfg.obfuscation.strength);
            println!("  Max latency: {:.0} ms", cfg.obfuscation.max_latency_ms);
            println!(
                "  Bucket:      {:.0} ms (dwell), {:.0} ms (flight)",
                cfg.obfuscation.dwell_bucket_ms, cfg.obfuscation.flight_bucket_ms
            );
            println!("  Noise σ:     {:.0} ms", cfg.obfuscation.noise_stddev_ms);
            return Ok(());
        }
        _ => {}
    }

    // Load config and apply CLI overrides.
    let mut cfg = AppConfig::load()?;

    if let Some(s) = cli.strength {
        cfg.obfuscation.strength = s.clamp(0.0, 1.0);
    }
    if let Some(m) = cli.max_latency {
        cfg.obfuscation.max_latency_ms = m.max(0.0);
    }
    if let Some(b) = cli.bucket {
        cfg.obfuscation.dwell_bucket_ms = b.max(1.0);
        cfg.obfuscation.flight_bucket_ms = b.max(1.0);
    }
    if let Some(n) = cli.noise {
        cfg.obfuscation.noise_stddev_ms = n.max(0.0);
    }

    // Determine enabled state.
    let enabled = !matches!(cli.command, Some(Commands::Off));

    // Daemonise if requested.
    if cli.daemon {
        daemonise()?;
    }

    // Print banner.
    log::info!("ShuffleKeys v{}", env!("CARGO_PKG_VERSION"));
    log::info!(
        "Obfuscation: {} | strength={:.1} | max_latency={:.0}ms | bucket={:.0}/{:.0}ms | noise_σ={:.0}ms",
        if enabled { "ON" } else { "OFF" },
        cfg.obfuscation.strength,
        cfg.obfuscation.max_latency_ms,
        cfg.obfuscation.dwell_bucket_ms,
        cfg.obfuscation.flight_bucket_ms,
        cfg.obfuscation.noise_stddev_ms,
    );

    // Build the obfuscation engine.
    let mut engine =
        ObfuscationEngine::new(cfg.obfuscation.clone(), cfg.advanced.clone());
    engine.enabled = enabled;

    if matches!(cli.command, Some(Commands::NewPersona)) {
        engine.regenerate_persona();
    }

    // Build scheduler.
    let scheduler = EventScheduler::new();

    // Set up signal handling for clean shutdown.
    install_signal_handlers();

    // Create the platform interceptor.
    let device_path = cli.device.as_deref().or(cfg.system.device.as_deref());
    let mut interceptor =
        platform::create_interceptor(device_path, cfg.system.auto_detect_keyboard)?;

    // Run the event loop.
    //
    // The interceptor's `run` method blocks reading from the grabbed
    // keyboard.  For each key event it calls our callback.  Inside the
    // callback we feed the event through the obfuscation engine and decide
    // whether to emit immediately or defer.
    //
    // Because the callback can't call back into the interceptor (borrow
    // rules), we collect deferred events and the `run` implementation
    // handles emitting events returned by the callback.  For truly
    // deferred events (non-zero delay), we use a simpler model: apply the
    // delay inline via a short busy-wait/sleep *before* returning the
    // event from the callback.

    interceptor.run(Box::new(move |raw_event: KeyEvent| {
        if !shufflekeys::is_running() {
            return Some(raw_event); // shutting down — pass through
        }

        let now_us = scheduler.now_us();
        let scheduled = engine.process(raw_event.clone(), now_us);

        // If the event should be emitted later, wait for it.
        if scheduled.emit_at_us > now_us {
            let wait_us = scheduled.emit_at_us - now_us;
            if wait_us > MIN_SLEEP_THRESHOLD_US {
                // Sleep for most of the delay.
                std::thread::sleep(std::time::Duration::from_micros(wait_us.saturating_sub(SLEEP_OVERHEAD_US)));
            }
            // Busy-wait for the final sub-ms.
            while scheduler.now_us() < scheduled.emit_at_us {
                std::hint::spin_loop();
            }
        }

        Some(scheduled.event)
    }))?;

    // Clean up.
    interceptor.stop()?;
    log::info!("ShuffleKeys stopped");
    Ok(())
}

/// Minimal daemonisation: fork, setsid, redirect stdio.
fn daemonise() -> anyhow::Result<()> {
    use nix::unistd::{fork, setsid, ForkResult};
    use std::os::unix::io::AsRawFd;
    use std::process;

    log::info!("Daemonising…");
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            println!("ShuffleKeys daemon started (pid {})", child);
            process::exit(0);
        }
        Ok(ForkResult::Child) => {
            setsid().context("setsid failed")?;
            // Redirect stdio to /dev/null.
            let devnull = std::fs::File::open("/dev/null")?;
            nix::unistd::dup2(devnull.as_raw_fd(), 0)?;
            nix::unistd::dup2(devnull.as_raw_fd(), 1)?;
            nix::unistd::dup2(devnull.as_raw_fd(), 2)?;
            Ok(())
        }
        Err(e) => anyhow::bail!("fork failed: {e}"),
    }
}

// ── Signal handling ────────────────────────────────────────────────────────

fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGINT, handle_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, handle_signal as *const () as libc::sighandler_t);
    }
}

extern "C" fn handle_signal(_sig: libc::c_int) {
    shufflekeys::stop();
}
