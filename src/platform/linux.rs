use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{bail, Context};
use evdev::{uinput::VirtualDeviceBuilder, Device, EventType, InputEvent, Key};

use super::{KeyEvent, KeyEventType, KeyboardInterceptor};

/// Linux keyboard interceptor using evdev (grab) + uinput (virtual device).
pub struct LinuxInterceptor {
    /// The grabbed physical device.
    device: Option<Device>,
    /// The uinput virtual device we emit through.
    virtual_device: Option<evdev::uinput::VirtualDevice>,
    /// Path for display purposes.
    device_path: PathBuf,
    /// Whether we currently hold the grab.
    grabbed: bool,
}

impl LinuxInterceptor {
    /// Create a new interceptor.
    ///
    /// - `device_path`: explicit `/dev/input/eventN`, or
    /// - `auto_detect`: scan sysfs for a likely keyboard device.
    pub fn new(device_path: Option<&str>, auto_detect: bool) -> anyhow::Result<Self> {
        let path = if let Some(p) = device_path {
            PathBuf::from(p)
        } else if auto_detect {
            Self::detect_keyboard().context("Failed to auto-detect keyboard device")?
        } else {
            bail!("No device path given and auto-detect is disabled");
        };

        log::info!("Opening keyboard device: {}", path.display());

        let mut device =
            Device::open(&path).with_context(|| format!("Cannot open {}", path.display()))?;

        // Grab exclusive access — prevents original events from reaching
        // applications.
        device
            .grab()
            .context("EVIOCGRAB failed — do you have permission? (try sudo or input group)")?;

        log::info!("Grabbed {}", path.display());

        // Build a virtual device that mirrors the physical keyboard's
        // capabilities.
        let virt = Self::create_virtual_device(&device)?;

        Ok(Self {
            device: Some(device),
            virtual_device: Some(virt),
            device_path: path,
            grabbed: true,
        })
    }

    /// Scan `/dev/input/` for a device that looks like a keyboard.
    fn detect_keyboard() -> anyhow::Result<PathBuf> {
        let mut best: Option<(PathBuf, String)> = None;

        for entry in fs::read_dir("/dev/input")? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if !name.starts_with("event") {
                continue;
            }

            // Try to open and check capabilities.
            if let Ok(dev) = Device::open(&path) {
                let supported = dev.supported_keys();
                if let Some(keys) = supported {
                    // A keyboard should support KEY_A (30) and KEY_ENTER (28).
                    let has_a = keys.contains(Key::KEY_A);
                    let has_enter = keys.contains(Key::KEY_ENTER);
                    let has_space = keys.contains(Key::KEY_SPACE);
                    if has_a && has_enter && has_space {
                        let dev_name = dev.name().unwrap_or("unknown").to_string();
                        log::debug!("Candidate keyboard: {} ({})", path.display(), dev_name);
                        // Prefer devices whose name contains "keyboard" (case-insensitive).
                        let name_lower = dev_name.to_lowercase();
                        if name_lower.contains("keyboard") || best.is_none() {
                            best = Some((path.clone(), dev_name));
                            if name_lower.contains("keyboard") {
                                break; // good enough
                            }
                        }
                    }
                }
            }
        }

        match best {
            Some((path, name)) => {
                log::info!("Auto-detected keyboard: {} ({})", path.display(), name);
                Ok(path)
            }
            None => bail!("No keyboard device found in /dev/input/"),
        }
    }

    /// Create a uinput virtual device mirroring the physical one.
    fn create_virtual_device(physical: &Device) -> anyhow::Result<evdev::uinput::VirtualDevice> {
        let mut builder = VirtualDeviceBuilder::new()?.name("shufflekeys-virtual-keyboard");

        // Copy supported event types and keys.
        if let Some(keys) = physical.supported_keys() {
            builder = builder.with_keys(&keys)?;
        }

        let mut virt = builder.build()?;
        log::info!(
            "Created virtual keyboard: {}",
            virt.get_syspath()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "unknown".into())
        );
        Ok(virt)
    }

    /// Emit a raw InputEvent through the virtual device.
    fn emit_raw_event(
        virt: &mut evdev::uinput::VirtualDevice,
        ev: &InputEvent,
    ) -> anyhow::Result<()> {
        virt.emit(&[*ev])?;
        Ok(())
    }

    /// Emit a KeyEvent through the virtual device.
    fn emit_key_event(
        virt: &mut evdev::uinput::VirtualDevice,
        event: &KeyEvent,
    ) -> anyhow::Result<()> {
        let value = match event.event_type {
            KeyEventType::Down => 1,
            KeyEventType::Up => 0,
            KeyEventType::Repeat => 2,
        };
                let ev = InputEvent::new(EventType::KEY, event.key_code, value);
                virt.emit(&[ev])?;
        
                Ok(())
            }
        }
        
        /// Linux modifier keys (evdev codes).
        pub fn is_modifier(key_code: u16) -> bool {
            const MODIFIER_CODES: &[u16] = &[
                29,  // KEY_LEFTCTRL
                42,  // KEY_LEFTSHIFT
                54,  // KEY_RIGHTSHIFT
                56,  // KEY_LEFTALT
                97,  // KEY_RIGHTCTRL
                100, // KEY_RIGHTALT
                125, // KEY_LEFTMETA
                126, // KEY_RIGHTMETA
            ];
            MODIFIER_CODES.contains(&key_code)
        }
        
        impl KeyboardInterceptor for LinuxInterceptor {
        
            fn run(
        
                &mut self,
        
                mut callback: Box<dyn FnMut(KeyEvent) -> Option<KeyEvent> + Send>,
        
            ) -> anyhow::Result<()> {
        
        
        let device = self.device.as_mut().context("Device not open")?;
        let virt = self.virtual_device.as_mut().context("Virtual device not open")?;

        log::info!("Entering event loop on {}", self.device_path.display());

        loop {
            // fetch_events blocks until events are available.
            let events: Vec<InputEvent> = match device.fetch_events() {
                Ok(evts) => evts.collect(),
                Err(e) => {
                    // ENODEV means the device was unplugged.
                    if e.raw_os_error() == Some(19) {
                        log::warn!("Device disconnected");
                        break;
                    }
                    // EAGAIN / interrupted — retry.
                    if e.raw_os_error() == Some(11) || e.raw_os_error() == Some(4) {
                        if !crate::is_running() {
                            break;
                        }
                        continue;
                    }
                    return Err(e.into());
                }
            };

            for ev in events {
                // We only care about EV_KEY events.
                if ev.event_type() != EventType::KEY {
                    // Pass through SYN and other events verbatim.
                    Self::emit_raw_event(virt, &ev)?;
                    continue;
                }

                let event_type = match ev.value() {
                    0 => KeyEventType::Up,
                    1 => KeyEventType::Down,
                    2 => KeyEventType::Repeat,
                    _ => continue,
                };

                let ts = ev.timestamp();
                let timestamp_us = ts
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as u64;

                let key_event = KeyEvent {
                    key_code: ev.code(),
                    event_type,
                    timestamp_us,
                };

                // Let the obfuscation engine decide what to emit.
                if let Some(modified) = callback(key_event) {
                    Self::emit_key_event(virt, &modified)?;
                }
            }
        }

        Ok(())
    }

    fn emit_event(&mut self, event: &KeyEvent) -> anyhow::Result<()> {
        let virt = self
            .virtual_device
            .as_mut()
            .context("Virtual device not open")?;

        let value = match event.event_type {
            KeyEventType::Down => 1,
            KeyEventType::Up => 0,
            KeyEventType::Repeat => 2,
        };

        let ev = InputEvent::new(EventType::KEY, event.key_code, value);
        virt.emit(&[ev])?;

        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        if self.grabbed {
            if let Some(ref mut dev) = self.device {
                let _ = dev.ungrab();
                log::info!("Released grab on {}", self.device_path.display());
            }
            self.grabbed = false;
        }
        // Dropping the virtual device destroys it.
        self.virtual_device.take();
        self.device.take();
        Ok(())
    }
}

impl Drop for LinuxInterceptor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
