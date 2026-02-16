//! Obfuscation engine: timing manipulation, configuration, and event scheduling.
//!
//! The engine takes raw keystroke events and applies quantisation, Gaussian
//! noise, and physiological clamping to produce obfuscated timing that defeats
//! biometric fingerprinting while remaining imperceptible to the user.

pub mod config;
pub mod obfuscation;
pub mod scheduler;
