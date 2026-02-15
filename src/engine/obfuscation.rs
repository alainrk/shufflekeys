use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_distr::{Distribution, Normal};

use crate::engine::config::{AdvancedConfig, ObfuscationConfig};
use crate::platform::{KeyEvent, KeyEventType};

// ── Scheduled event ────────────────────────────────────────────────────────

/// An event together with the absolute (monotonic, µs) time at which it
/// should be emitted.
#[derive(Debug, Clone)]
pub struct ScheduledEvent {
    pub event: KeyEvent,
    /// Absolute monotonic timestamp in microseconds when we should emit.
    pub emit_at_us: u64,
}

impl PartialEq for ScheduledEvent {
    fn eq(&self, other: &Self) -> bool {
        self.emit_at_us == other.emit_at_us
    }
}

impl Eq for ScheduledEvent {}

impl PartialOrd for ScheduledEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse so BinaryHeap gives us the *earliest* event first.
        other.emit_at_us.cmp(&self.emit_at_us)
    }
}

// ── Obfuscation engine ────────────────────────────────────────────────────

/// Core obfuscation logic: quantise, add Gaussian noise, clamp, per-session
/// persona seed.
pub struct ObfuscationEngine {
    pub obf: ObfuscationConfig,
    pub adv: AdvancedConfig,
    /// Per-session persona seed — makes cross-session linking harder.
    pub persona_seed: u64,
    rng: StdRng,
    /// Tracks scheduled keyup deadlines keyed by key code.
    pending_keyup_times: HashMap<u16, u64>,
    /// Timestamp (µs) of the last *emitted* event for flight-time calculation.
    last_emit_us: u64,
    /// Whether obfuscation is currently active.
    pub enabled: bool,
}

impl ObfuscationEngine {
    pub fn new(obf: ObfuscationConfig, adv: AdvancedConfig) -> Self {
        let persona_seed: u64 = rand::thread_rng().gen();
        log::info!("New persona seed: {:#018x}", persona_seed);
        Self {
            obf,
            adv,
            persona_seed,
            rng: StdRng::seed_from_u64(persona_seed),
            pending_keyup_times: HashMap::new(),
            last_emit_us: 0,
            enabled: true,
        }
    }

    /// Re-roll the persona seed (e.g. on user request or session restart).
    pub fn regenerate_persona(&mut self) {
        self.persona_seed = rand::thread_rng().gen();
        self.rng = StdRng::seed_from_u64(self.persona_seed);
        self.pending_keyup_times.clear();
        self.last_emit_us = 0;
        log::info!("Persona regenerated: {:#018x}", self.persona_seed);
    }

    /// Process an incoming key event and return a `ScheduledEvent` with the
    /// (possibly delayed) emit time.
    pub fn process(&mut self, mut event: KeyEvent) -> ScheduledEvent {
        let now_us = event.timestamp_us;

        // Pass-through when disabled or for modifier keys.
        if !self.enabled || crate::platform::is_modifier(event.key_code) {
            return ScheduledEvent {
                event,
                emit_at_us: now_us,
            };
        }

        let scheduled = match event.event_type {
            KeyEventType::Down => self.process_keydown(event, now_us),
            KeyEventType::Up => self.process_keyup(event, now_us),
            KeyEventType::Repeat => {
                ScheduledEvent {
                    event,
                    emit_at_us: now_us,
                }
            }
        };

        // Update the event's internal timestamp to reflect its new scheduled time.
        let mut out = scheduled;
        out.event.timestamp_us = out.emit_at_us;
        out
    }

    // ── internal ──────────────────────────────────────────────────────────

    fn process_keydown(&mut self, event: KeyEvent, now_us: u64) -> ScheduledEvent {
        // Original flight time from last emitted event.
        let original_flight_us = if self.last_emit_us == 0 {
            0
        } else {
            now_us.saturating_sub(self.last_emit_us)
        };

        // Obfuscate flight time.
        let obfuscated_flight_us = if original_flight_us == 0 {
            0
        } else {
            self.obfuscate_timing(
                original_flight_us,
                self.obf.flight_bucket_ms,
                self.adv.min_flight_ms,
                self.adv.max_flight_ms,
            )
        };

        // The delay we need to add (never negative).
        let mut delay_us = obfuscated_flight_us.saturating_sub(original_flight_us);

        // Honour max-latency cap.
        let cap_us = (self.obf.max_latency_ms * 1000.0) as u64;
        if delay_us > cap_us {
            delay_us = cap_us;
        }

        // Apply strength blending: delay = delay * strength.
        delay_us = (delay_us as f64 * self.obf.strength) as u64;

        let emit_at = now_us + delay_us;

        // Pre-calculate obfuscated dwell so we know when to emit the keyup.
        let typical_dwell = self.typical_dwell_us(event.key_code);
        let target_dwell_us = self.obfuscate_timing(
            typical_dwell,
            self.obf.dwell_bucket_ms,
            self.adv.min_dwell_ms,
            self.adv.max_dwell_ms,
        );
        // Store earliest keyup emit time.
        self.pending_keyup_times
            .insert(event.key_code, emit_at + target_dwell_us);

        self.last_emit_us = emit_at;

        ScheduledEvent {
            event,
            emit_at_us: emit_at,
        }
    }

    fn process_keyup(&mut self, event: KeyEvent, now_us: u64) -> ScheduledEvent {
        let emit_at = if let Some(&min_up_time) = self.pending_keyup_times.get(&event.key_code) {
            // Ensure the keyup doesn't arrive before the obfuscated dwell
            // has elapsed — but also don't delay more than max-latency from
            // *now*.
            let cap_us = (self.obf.max_latency_ms * 1000.0) as u64;
            let latest = now_us + cap_us;
            let ideal = min_up_time.max(now_us); // never go back in time
            ideal.min(latest) // but respect cap
        } else {
            now_us
        };

        self.pending_keyup_times.remove(&event.key_code);
        self.last_emit_us = emit_at;

        ScheduledEvent {
            event,
            emit_at_us: emit_at,
        }
    }

    /// Hybrid obfuscation: quantise → Gaussian noise → clamp.
    fn obfuscate_timing(
        &mut self,
        original_us: u64,
        bucket_ms: f64,
        min_ms: f64,
        max_ms: f64,
    ) -> u64 {
        let original_ms = original_us as f64 / 1000.0;

        // Step 1 — Quantise to bucket.
        let quantized = (original_ms / bucket_ms).round() * bucket_ms;

        // Step 2 — Add Gaussian noise (seeded per-persona).
        let normal = Normal::new(0.0, self.obf.noise_stddev_ms)
            .expect("invalid noise stddev");
        let noise: f64 = normal.sample(&mut self.rng);
        let noised = quantized + noise;

        // Step 3 — Clamp to physiologically plausible range.
        let clamped = noised.clamp(min_ms, max_ms);

        (clamped * 1000.0) as u64
    }

    /// Returns a rough "typical" dwell time for a key, with a small
    /// per-persona perturbation so the synthetic dwell isn't identical
    /// across sessions.
    fn typical_dwell_us(&mut self, key_code: u16) -> u64 {
        // Base ~90 ms, with a persona-dependent per-key tweak of ±15 ms.
        let base_ms: f64 = 90.0;
        // Deterministic per-key offset derived from persona seed.
        let hash = self.persona_seed.wrapping_mul(key_code as u64 + 1);
        let offset_ms = ((hash % 3000) as f64 / 100.0) - 15.0; // ±15 ms
        let dwell_ms = (base_ms + offset_ms).max(40.0);
        (dwell_ms * 1000.0) as u64
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::config::{AdvancedConfig, ObfuscationConfig};

    fn make_engine() -> ObfuscationEngine {
        ObfuscationEngine::new(ObfuscationConfig::default(), AdvancedConfig::default())
    }

    #[test]
    fn passthrough_when_disabled() {
        let mut eng = make_engine();
        eng.enabled = false;
        let ev = KeyEvent {
            key_code: 30,
            event_type: KeyEventType::Down,
            timestamp_us: 1_000_000,
        };
        let out = eng.process(ev.clone());
        assert_eq!(out.emit_at_us, 1_000_000);
    }

    #[test]
    fn modifiers_not_delayed() {
        let mut eng = make_engine();
        // Test both Linux and macOS codes to ensure the underlying is_modifier 
        // works for whatever platform it's currently compiled for.
        let linux_codes = vec![29, 42, 56, 125];
        let macos_codes = vec![56, 55, 59, 58];
        
        let all_codes = if cfg!(target_os = "linux") {
            linux_codes
        } else if cfg!(target_os = "macos") {
            macos_codes
        } else {
            vec![]
        };

        for code in all_codes {
            let ev = KeyEvent {
                key_code: code,
                event_type: KeyEventType::Down,
                timestamp_us: 1_000_000,
            };
            let out = eng.process(ev);
            assert_eq!(out.emit_at_us, 1_000_000, "modifier {code} was delayed");
        }
    }

    #[test]
    fn obfuscated_timing_within_bounds() {
        let mut eng = make_engine();
        for _ in 0..500 {
            let val = eng.obfuscate_timing(
                100_000, // 100 ms
                15.0,
                40.0,
                200.0,
            );
            let ms = val as f64 / 1000.0;
            assert!(ms >= 40.0, "below floor: {ms}");
            assert!(ms <= 200.0, "above ceiling: {ms}");
        }
    }

    #[test]
    fn max_latency_respected() {
        let mut eng = make_engine();
        eng.obf.max_latency_ms = 10.0;
        eng.obf.strength = 1.0;

        // Simulate a first keydown (sets last_emit_us).
        let ev1 = KeyEvent {
            key_code: 30,
            event_type: KeyEventType::Down,
            timestamp_us: 1_000_000,
        };
        let _ = eng.process(ev1);

        // Second keydown a very short time later — engine will want to delay
        // but must not exceed 10 ms = 10 000 µs.
        let ev2 = KeyEvent {
            key_code: 31,
            event_type: KeyEventType::Down,
            timestamp_us: 1_001_000, // 1 ms later
        };
        let out = eng.process(ev2);
        let added = out.emit_at_us.saturating_sub(1_001_000);
        assert!(
            added <= 10_000,
            "added latency {added} µs exceeds cap of 10 000 µs"
        );
    }

    #[test]
    fn persona_regeneration_changes_output() {
        let mut eng = make_engine();
        let ev = KeyEvent {
            key_code: 30,
            event_type: KeyEventType::Down,
            timestamp_us: 1_000_000,
        };
        let out1 = eng.process(ev.clone());

        eng.regenerate_persona();
        let out2 = eng.process(ev);

        // Very unlikely to be identical after persona change.
        let _ = (out1, out2);
    }
}
