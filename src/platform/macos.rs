use std::os::raw::c_void;
use std::ptr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::{bail, Result};
use core_foundation::base::TCFType;
use core_foundation::mach_port::{CFMachPort, CFMachPortRef};
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop, CFRunLoopSource};
use core_graphics::event::{
    CGEvent, CGEventField, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CGKeyCode,
};
use foreign_types::ForeignType;

use super::{KeyEvent, KeyEventType, KeyboardInterceptor};

/// macOS keyboard interceptor using CGEventTap.
pub struct MacosInterceptor {
    /// The event tap mach port.
    tap: Option<CFMachPort>,
    /// We need to keep the run loop source alive.
    source: Option<CFRunLoopSource>,
    /// Sender to the worker thread.
    worker_tx: Option<Sender<KeyEvent>>,
}

/// Context passed to the C callback.
struct TapContext {
    tx: Sender<KeyEvent>,
}

// ── FFI for Core Graphics / Core Foundation ──────────────────────────────────

type CGEventTapProxy = *mut c_void;

type CGEventTapCallback = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    etype: CGEventType,
    event: core_graphics::sys::CGEventRef,
    user_info: *mut c_void,
) -> core_graphics::sys::CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: CGEventTapLocation,
        place: CGEventTapPlacement,
        options: CGEventTapOptions,
        eventsOfInterest: u64,
        callback: CGEventTapCallback,
        userInfo: *mut c_void,
    ) -> CFMachPortRef;

    fn CGEventGetTimestamp(event: core_graphics::sys::CGEventRef) -> u64;
    fn CGEventSetTimestamp(event: core_graphics::sys::CGEventRef, timestamp: u64);
}

// Constants that might be missing or named differently in core-graphics crate.
const K_CG_KEYBOARD_EVENT_KEYCODE: CGEventField = 9;
const K_CG_KEYBOARD_EVENT_AUTOREPEAT: CGEventField = 8;
const K_CG_EVENT_SOURCE_USER_DATA: CGEventField = 42;

const SHUFFLEKEYS_MAGIC: i64 = 0x534b4559; // "SKEY"

impl MacosInterceptor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            tap: None,
            source: None,
            worker_tx: None,
        })
    }
}

/// macOS modifier keys (CGKeyCodes).
pub fn is_modifier(key_code: u16) -> bool {
    // 54: R Command, 55: L Command
    // 56: L Shift, 60: R Shift
    // 58: L Option, 61: R Option
    // 59: L Control, 62: R Control
    // 57: Caps Lock, 63: Function
    matches!(key_code, 54..=63)
}

impl KeyboardInterceptor for MacosInterceptor {
    fn run(
        &mut self,
        mut callback: Box<dyn FnMut(KeyEvent) -> Option<KeyEvent> + Send>,
    ) -> Result<()> {
        log::info!("Starting macOS event tap");

        let (tx, rx): (Sender<KeyEvent>, Receiver<KeyEvent>) = mpsc::channel();
        self.worker_tx = Some(tx.clone());

        let mut tap_context = TapContext { tx };

        let worker_handle = thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                if let Some(modified) = callback(event) {
                    let _ = Self::post_event(&modified);
                }
            }
        });

        // 2. Define the tap callback.
        unsafe extern "C" fn raw_callback(
            _proxy: CGEventTapProxy,
            etype: CGEventType,
            event: core_graphics::sys::CGEventRef,
            user_info: *mut c_void,
        ) -> core_graphics::sys::CGEventRef {
            let context = &mut *(user_info as *mut TapContext);
            let event_obj = std::mem::ManuallyDrop::new(CGEvent::from_ptr(event));

            // Recursion prevention: if this is an event we posted, let it through 
            // without intercepting it again.
            if event_obj.get_integer_value_field(K_CG_EVENT_SOURCE_USER_DATA) == SHUFFLEKEYS_MAGIC {
                return event;
            }

            let is_repeat = event_obj.get_integer_value_field(K_CG_KEYBOARD_EVENT_AUTOREPEAT) != 0;

            let event_type = match etype {
                CGEventType::KeyDown => {
                    if is_repeat {
                        KeyEventType::Repeat
                    } else {
                        KeyEventType::Down
                    }
                }
                CGEventType::KeyUp => KeyEventType::Up,
                _ => return event,
            };

            let key_code =
                event_obj.get_integer_value_field(K_CG_KEYBOARD_EVENT_KEYCODE) as u16;

            let timestamp_ns = CGEventGetTimestamp(event);
            let timestamp_us = timestamp_ns / 1000;

            let key_event = KeyEvent {
                key_code,
                event_type,
                timestamp_us,
            };

            if let Err(_) = context.tx.send(key_event) {
                return event;
            }

            ptr::null_mut()
        }

        let mask = (1u64 << CGEventType::KeyDown as u64) | (1u64 << CGEventType::KeyUp as u64);
        let tap_port_ref = unsafe {
            CGEventTapCreate(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                mask,
                raw_callback,
                &mut tap_context as *mut TapContext as *mut c_void,
            )
        };

        if tap_port_ref.is_null() {
            bail!("Failed to create CGEventTap. Do you have 'Input Monitoring' / 'Accessibility' permissions?");
        }

        let tap = unsafe { CFMachPort::wrap_under_create_rule(tap_port_ref) };

        // 4. Add to run loop.
        let source = tap.create_runloop_source(0).map_err(|_| anyhow::anyhow!("Failed to create runloop source"))?;
        let run_loop = CFRunLoop::get_current();
        unsafe {
            run_loop.add_source(&source, kCFRunLoopCommonModes);
        }

        // 5. Add a timer to check for shutdown and stop the run loop.
        // This ensures Ctrl+C works even if no keys are being pressed.
        extern "C" fn timer_callback(
            _timer: core_foundation::runloop::CFRunLoopTimerRef,
            _info: *mut std::os::raw::c_void,
        ) {
            if !crate::is_running() {
                CFRunLoop::get_current().stop();
            }
        }

        let timer = unsafe {
            let timer_ref = core_foundation::runloop::CFRunLoopTimerCreate(
                core_foundation::base::kCFAllocatorDefault,
                0.0, // fire now
                0.1, // every 100ms
                0,
                0,
                timer_callback,
                std::ptr::null_mut(),
            );
            core_foundation::runloop::CFRunLoopTimer::wrap_under_create_rule(timer_ref)
        };
        unsafe {
            run_loop.add_timer(&timer, kCFRunLoopCommonModes);
        }

        self.tap = Some(tap);
        self.source = Some(source);

        log::info!("macOS event tap active. Press Ctrl+C to stop.");

        // This blocks.
        CFRunLoop::run_current();

        let _ = worker_handle; 

        Ok(())
    }

    fn emit_event(&mut self, event: &KeyEvent) -> Result<()> {
        Self::post_event(event)
    }

    fn stop(&mut self) -> Result<()> {
        log::info!("Stopping macOS interceptor");
        if let Some(source) = self.source.take() {
            let run_loop = CFRunLoop::get_current();
            unsafe {
                run_loop.remove_source(&source, kCFRunLoopCommonModes);
            }
        }
        self.tap.take();
        self.worker_tx.take(); // Close channel
        CFRunLoop::get_current().stop();
        Ok(())
    }
}

impl MacosInterceptor {
    fn post_event(event: &KeyEvent) -> Result<()> {
        let source = core_graphics::event_source::CGEventSource::new(
            core_graphics::event_source::CGEventSourceStateID::HIDSystemState,
        )
        .map_err(|_| anyhow::anyhow!("Failed source"))?;

        let mut cg_event = CGEvent::new_keyboard_event(
            source,
            event.key_code as CGKeyCode,
            matches!(event.event_type, KeyEventType::Down | KeyEventType::Repeat),
        )
        .map_err(|_| anyhow::anyhow!("Failed event"))?;

        if event.event_type == KeyEventType::Repeat {
            cg_event.set_integer_value_field(K_CG_KEYBOARD_EVENT_AUTOREPEAT, 1);
        }

        cg_event.set_integer_value_field(K_CG_EVENT_SOURCE_USER_DATA, SHUFFLEKEYS_MAGIC);

        unsafe {
            CGEventSetTimestamp(cg_event.as_ptr(), event.timestamp_us * 1000);
        }

        cg_event.post(CGEventTapLocation::HID);
        Ok(())
    }
}

impl Drop for MacosInterceptor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
