use std::{
    ops::Sub,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub(crate) struct ReferenceInstant {
    #[cfg(not(feature = "strong"))]
    instant: Instant,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct RelativeInstant {
    #[cfg(feature = "strong")]
    relative: Instant,
    #[cfg(not(feature = "strong"))]
    relative: Duration,
}

impl ReferenceInstant {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(not(feature = "strong"))]
            instant: Instant::now(),
        }
    }

    pub(crate) fn now(&self) -> RelativeInstant {
        RelativeInstant {
            #[cfg(feature = "strong")]
            relative: Instant::now(),
            #[cfg(not(feature = "strong"))]
            relative: self.instant.elapsed(),
        }
    }
}

impl RelativeInstant {
    #[cfg(not(feature = "strong"))]
    pub(crate) const ENCODED_LEN: usize = 12;

    #[cfg(not(feature = "strong"))]
    pub(crate) fn encode(&self) -> [u8; Self::ENCODED_LEN] {
        let mut buf = [0u8; Self::ENCODED_LEN];
        let (secs, nanos) = buf.split_at_mut(8);
        secs.copy_from_slice(&self.relative.as_secs().to_ne_bytes());
        nanos.copy_from_slice(&self.relative.subsec_nanos().to_ne_bytes());

        buf
    }

    #[cfg(not(feature = "strong"))]
    pub(crate) fn decode(buf: &[u8; Self::ENCODED_LEN]) -> Option<Self> {
        let (secs, nanos) = buf.split_at(8);
        let secs = u64::from_ne_bytes(secs.try_into().unwrap());
        let nanos = u32::from_ne_bytes(nanos.try_into().unwrap());

        let relative = Duration::from_secs(secs).checked_add(Duration::from_nanos(nanos.into()))?;
        Some(Self { relative })
    }
}

impl Sub<RelativeInstant> for RelativeInstant {
    type Output = Duration;

    fn sub(self, rhs: RelativeInstant) -> Self::Output {
        #[cfg(feature = "strong")]
        {
            self.relative.saturating_duration_since(rhs.relative)
        }

        #[cfg(not(feature = "strong"))]
        {
            self.relative.saturating_sub(rhs.relative)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ReferenceInstant;
    #[cfg(not(feature = "strong"))]
    use super::RelativeInstant;

    #[test]
    fn passing_time() {
        let reference = ReferenceInstant::new();
        let a = reference.now();
        let b = reference.now();

        assert!(b - a <= Duration::from_millis(10));
    }

    #[test]
    fn wrong_order() {
        let reference = ReferenceInstant::new();
        let a = reference.now();
        let b = reference.now();

        assert_eq!(a - b, Duration::ZERO);
    }

    #[test]
    #[cfg(not(feature = "strong"))]
    fn encode_decode() {
        let reference = ReferenceInstant::new();
        let a1 = reference.now();
        let a2 = RelativeInstant::decode(&a1.encode()).unwrap();

        assert_eq!(a1.relative, a2.relative);
    }
}
