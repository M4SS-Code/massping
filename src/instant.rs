use std::{mem, ops::Sub, time::Duration};

#[derive(Copy, Clone)]
pub struct Instant {
    spec: libc::timespec,
}

impl Instant {
    pub fn now() -> Self {
        let mut spec = unsafe { mem::zeroed() };
        unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut spec as *mut _);
        }
        Self { spec }
    }

    pub const ENCODED_LEN: usize = mem::size_of::<libc::timespec>();

    pub fn encode(&self) -> [u8; Self::ENCODED_LEN] {
        // SAFETY: transmuting between items of the same length is fine
        unsafe { mem::transmute_copy(&self.spec) }
    }

    pub fn decode(bytes: &[u8; Self::ENCODED_LEN]) -> Option<Self> {
        // SAFETY: transmuting between items of the same length is fine
        let spec: libc::timespec = unsafe { mem::transmute_copy(bytes) };

        if spec.tv_sec >= 0 && spec.tv_nsec >= 0 {
            Some(Self { spec })
        } else {
            None
        }
    }
}

impl From<Instant> for Duration {
    fn from(instant: Instant) -> Self {
        Duration::new(instant.spec.tv_sec as _, instant.spec.tv_nsec as _)
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, other: Instant) -> Self::Output {
        Duration::from(self).saturating_sub(Duration::from(other))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Instant;

    #[test]
    fn passing_time() {
        let a = Instant::now();
        let b = Instant::now();

        assert!(b - a <= Duration::from_millis(10));
    }

    #[test]
    fn wrong_order() {
        let a = Instant::now();
        let b = Instant::now();

        assert_eq!(a - b, Duration::ZERO);
    }

    #[test]
    fn encode_decode() {
        let a1 = Instant::now();
        let a1_duration = Duration::from(a1);

        let a2 = Instant::decode(a1.encode()).unwrap();
        let a2_duration = Duration::from(a2);

        assert_eq!(a1_duration, a2_duration);
    }
}
