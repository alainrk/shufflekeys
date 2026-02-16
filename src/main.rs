use anyhow::Context;
use clap::Parser;

use shufflekeys::cli::{Cli, Commands};
use shufflekeys::engine::config::AppConfig;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Daemonise as early as possible if requested.
    // SAFETY: No threads are spawned and no global locks are held at this point,
    // making the fork() operation safe in the Rust environment.
    if cli.daemon {
        daemonise()?;
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

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
    if let Some(ref dev) = cli.device {
        cfg.system.device = Some(dev.clone());
    }

    let enabled = !matches!(cli.command, Some(Commands::Off));
    let new_persona = matches!(cli.command, Some(Commands::NewPersona));

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

    // Set up signal handling for clean shutdown.
    install_signal_handlers();

    // Arm the running flag before entering the event loop.
    shufflekeys::restart();

    // Run the engine (blocks until shutdown).
    shufflekeys::run_engine(cfg, enabled, new_persona)?;

    log::info!("ShuffleKeys stopped");
    Ok(())
}

/// Minimal daemonisation: fork, setsid, redirect stdio.
fn daemonise() -> anyhow::Result<()> {
    use nix::unistd::{fork, setsid, ForkResult};
    use std::os::unix::io::AsRawFd;
    use std::process;

    log::info!("Daemonising…");
    // SAFETY: No other threads are running at this point (pre-event-loop).
    // fork() duplicates the process; the parent exits immediately and the
    // child calls setsid to detach from the terminal.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            println!("ShuffleKeys daemon started (pid {child})");
            process::exit(0);
        }
        Ok(ForkResult::Child) => {
            setsid().context("setsid failed")?;
            let devnull = std::fs::File::open("/dev/null")?;
            nix::unistd::dup2(devnull.as_raw_fd(), 0)?;
            nix::unistd::dup2(devnull.as_raw_fd(), 1)?;
            nix::unistd::dup2(devnull.as_raw_fd(), 2)?;
            Ok(())
        }
        Err(e) => anyhow::bail!("fork failed: {e}"),
    }
}

fn install_signal_handlers() {
    // SAFETY: handle_signal only sets an AtomicBool which is async-signal-safe.
    // Registered before any threads are spawned.
    unsafe {
        libc::signal(
            libc::SIGINT,
            handle_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            handle_signal as *const () as libc::sighandler_t,
        );
    }
}

extern "C" fn handle_signal(_sig: libc::c_int) {
    shufflekeys::stop();
}
