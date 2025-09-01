//! Error handling for embedded SHNN systems
//!
//! This module provides lightweight error handling optimized for no-std environments.

use core::fmt;

/// Result type for embedded operations
pub type EmbeddedResult<T> = Result<T, EmbeddedError>;

/// Embedded error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedError {
    /// Memory allocation failed
    OutOfMemory,
    /// Invalid configuration (preferred canonical name)
    InvalidConfig,
    /// Hardware error
    HardwareError,
    /// Timeout error
    Timeout,
    /// Invalid neuron ID
    InvalidNeuronId,
    /// Buffer overflow (preferred canonical name)
    BufferOverflow,
    /// Arithmetic overflow
    ArithmeticOverflow,
    /// Real-time constraint violation
    RealTimeViolation,

    // Compatibility aliases used across modules (map to canonical semantics):
    /// Buffer at capacity (alias of BufferOverflow)
    BufferFull,
    /// Invalid configuration (alias of InvalidConfig)
    InvalidConfiguration,
    /// Invalid index into a bounded container
    InvalidIndex,
}

impl fmt::Display for EmbeddedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfMemory => write!(f, "Out of memory"),
            Self::InvalidConfig | Self::InvalidConfiguration => write!(f, "Invalid configuration"),
            Self::HardwareError => write!(f, "Hardware error"),
            Self::Timeout => write!(f, "Operation timed out"),
            Self::InvalidNeuronId => write!(f, "Invalid neuron ID"),
            Self::BufferOverflow | Self::BufferFull => write!(f, "Buffer overflow"),
            Self::ArithmeticOverflow => write!(f, "Arithmetic overflow"),
            Self::RealTimeViolation => write!(f, "Real-time constraint violation"),
            Self::InvalidIndex => write!(f, "Invalid index"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EmbeddedError {}