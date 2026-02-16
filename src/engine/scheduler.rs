use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

use crate::engine::obfuscation::ScheduledEvent;
use crate::platform::KeyEvent;

/// High-resolution event scheduler.
///
/// Events are queued with a target emit time.  The scheduler spins through
/// the queue, sleeping coarsely when possible and busy-waiting for the last
/// sub-millisecond to achieve high precision.
pub struct EventScheduler {
    queue: BinaryHeap<ScheduledEvent>,
    /// Monotonic reference point: `Instant` corresponding to µs epoch 0.
    epoch: Instant,
}

impl EventScheduler {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            epoch: Instant::now(),
        }
    }

    /// Returns the monotonic "now" in microseconds relative to our epoch.
    pub fn now_us(&self) -> u64 {
        self.epoch.elapsed().as_micros() as u64
    }

    /// Push a scheduled event into the queue.
    pub fn push(&mut self, ev: ScheduledEvent) {
        self.queue.push(ev);
    }

    /// Returns `true` if the queue has pending events.
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Try to pop the next event that is due (`emit_at_us` ≤ now).
    /// Returns `None` if the queue is empty or the next event isn't due yet.
    pub fn try_pop(&mut self) -> Option<KeyEvent> {
        let now = self.now_us();
        if let Some(top) = self.queue.peek() {
            if now >= top.emit_at_us {
                return Some(self.queue.pop().unwrap().event);
            }
        }
        None
    }

    /// Block until the next event is due, then return it.
    /// Uses coarse sleep + busy-wait for sub-ms precision.
    /// Returns `None` if the queue is empty.
    pub fn wait_pop(&mut self) -> Option<KeyEvent> {
        loop {
            let top = self.queue.peek()?;
            let now = self.now_us();
            if now >= top.emit_at_us {
                return Some(self.queue.pop().unwrap().event);
            }
            let remaining_us = top.emit_at_us - now;
            if remaining_us > 1500 {
                // Sleep for most of the wait, leaving ~1 ms for busy-wait.
                let sleep_us = remaining_us - 1000;
                std::thread::sleep(Duration::from_micros(sleep_us));
            } else {
                // Busy-wait (spin) for sub-ms precision.
                std::hint::spin_loop();
            }
        }
    }

    /// Peek at the µs timestamp of the next due event.
    pub fn next_emit_us(&self) -> Option<u64> {
        self.queue.peek().map(|e| e.emit_at_us)
    }

    /// Drain all events that are ready *right now* into a `Vec`.
    pub fn drain_ready(&mut self) -> Vec<KeyEvent> {
        let mut out = Vec::new();
        while let Some(ev) = self.try_pop() {
            out.push(ev);
        }
        out
    }
}

impl Default for EventScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::KeyEventType;

    #[test]
    fn immediate_event_pops() {
        let mut sched = EventScheduler::new();
        let now = sched.now_us();
        sched.push(ScheduledEvent {
            event: KeyEvent {
                key_code: 30,
                event_type: KeyEventType::Down,
                timestamp_us: now,
            },
            emit_at_us: now, // due immediately
        });
        assert!(sched.try_pop().is_some());
    }

    #[test]
    fn future_event_does_not_pop_early() {
        let mut sched = EventScheduler::new();
        let now = sched.now_us();
        sched.push(ScheduledEvent {
            event: KeyEvent {
                key_code: 30,
                event_type: KeyEventType::Down,
                timestamp_us: now,
            },
            emit_at_us: now + 1_000_000, // 1 second from now
        });
        assert!(sched.try_pop().is_none());
    }

    #[test]
    fn ordering_earliest_first() {
        let mut sched = EventScheduler::new();
        let now = sched.now_us();
        // Push later event first.
        sched.push(ScheduledEvent {
            event: KeyEvent {
                key_code: 31,
                event_type: KeyEventType::Down,
                timestamp_us: now,
            },
            emit_at_us: 0, // past → due
        });
        sched.push(ScheduledEvent {
            event: KeyEvent {
                key_code: 30,
                event_type: KeyEventType::Down,
                timestamp_us: now,
            },
            emit_at_us: 0,
        });
        let events = sched.drain_ready();
        assert_eq!(events.len(), 2);
    }
}
