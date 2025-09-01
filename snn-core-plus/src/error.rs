use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedError {
    // Generic errors (as requested)
    Alloc,
    Capacity,
    InvalidInput(&'static str),
    NotSupported(&'static str),
    Other(&'static str),
    // Compatibility aliases expected by embedded_* modules
    BufferFull,
    InvalidIndex,
    InvalidConfiguration,
}

impl fmt::Display for EmbeddedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmbeddedError::Alloc => write!(f, "allocation error"),
            EmbeddedError::Capacity => write!(f, "capacity exceeded"),
            EmbeddedError::InvalidInput(msg) => write!(f, "invalid input: {}", msg),
            EmbeddedError::NotSupported(msg) => write!(f, "not supported: {}", msg),
            EmbeddedError::Other(msg) => write!(f, "error: {}", msg),
            EmbeddedError::BufferFull => write!(f, "buffer full"),
            EmbeddedError::InvalidIndex => write!(f, "invalid index"),
            EmbeddedError::InvalidConfiguration => write!(f, "invalid configuration"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EmbeddedError {}

pub type EmbeddedResult<T, E = EmbeddedError> = core::result::Result<T, E>;

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;
    #[test]
    fn display_messages() {
        assert_eq!(format!("{}", EmbeddedError::Alloc), "allocation error");
        assert_eq!(format!("{}", EmbeddedError::Capacity), "capacity exceeded");
        assert_eq!(format!("{}", EmbeddedError::InvalidInput("bad")), "invalid input: bad");
        assert_eq!(format!("{}", EmbeddedError::NotSupported("x")), "not supported: x");
        assert_eq!(format!("{}", EmbeddedError::Other("oops")), "error: oops");
    }

    #[test]
    fn result_round_trip() {
        fn may_fail(ok: bool) -> EmbeddedResult<u32> {
            if ok { Ok(7) } else { Err(EmbeddedError::Other("fail")) }
        }
        assert_eq!(may_fail(true).unwrap(), 7);
        assert!(may_fail(false).is_err());
    }
}