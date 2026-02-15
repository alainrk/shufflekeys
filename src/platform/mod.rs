#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

// ── Cross-platform types ───────────────────────────────────────────────────

/// Keyboard event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventType {
    Down,
    Up,
    Repeat,
}

/// A single keyboard event.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// Linux: `input_event.code`; macOS: CGKeyCode.
    pub key_code: u16,
    pub event_type: KeyEventType,
    /// Original event timestamp in µs (monotonic, platform-specific epoch).
    pub timestamp_us: u64,
}

/// Trait that every platform interceptor must implement.
///
/// The interceptor grabs the real keyboard, feeds incoming events through a
/// callback (which returns an optional modified event), and emits the result
/// through a virtual device.
pub trait KeyboardInterceptor {
    /// Start the interception loop.
    ///
    /// `callback` receives each raw event and returns:
    /// - `Some(event)` → emit the (possibly modified) event immediately,
    /// - `None`        → event has been queued by the engine; the caller
    ///                    will emit it later via `emit_event`.
    fn run(
        &mut self,
        callback: Box<dyn FnMut(KeyEvent) -> Option<KeyEvent> + Send>,
    ) -> anyhow::Result<()>;

    /// Emit a single synthetic key event through the virtual device.
    fn emit_event(&mut self, event: &KeyEvent) -> anyhow::Result<()>;

    /// Release the grab and clean up.
    fn stop(&mut self) -> anyhow::Result<()>;
}

/// Returns true if the key code is a modifier key on the current platform.
pub fn is_modifier(key_code: u16) -> bool {
    #[cfg(target_os = "linux")]
    return linux::is_modifier(key_code);

    #[cfg(target_os = "macos")]
    return macos::is_modifier(key_code);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = key_code;
        false
    }
}

/// Auto-detect and create the appropriate interceptor for the current OS.
pub fn create_interceptor(
    device_path: Option<&str>,
    auto_detect: bool,
) -> anyhow::Result<Box<dyn KeyboardInterceptor>> {
    #[cfg(target_os = "linux")]
    {
        let interceptor = linux::LinuxInterceptor::new(device_path, auto_detect)?;
        Ok(Box::new(interceptor))
    }

    #[cfg(target_os = "macos")]
    {
        let _ = (device_path, auto_detect);
        let interceptor = macos::MacosInterceptor::new()?;
        Ok(Box::new(interceptor))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (device_path, auto_detect);
        anyhow::bail!("Unsupported platform");
    }
}
