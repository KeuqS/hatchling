//! A run's bounds: part of its shape, never optional.
//!
//! Every run gathers at most so many sources, and finishes within so many
//! seconds. There is no unbounded run: [`Bounds`] has no "off".

/// The most sources any run may gather.
pub const MAX_SOURCES: usize = 32;

/// The longest any run may take, in seconds.
pub const MAX_RUN_SECS: i64 = 3600;

/// How many sources a run may gather, and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    sources: usize,
    secs: i64,
}

impl Bounds {
    /// The standard bounds: 8 sources, 15 minutes.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            sources: 8,
            secs: 900,
        }
    }

    /// At most `sources` sources and `secs` seconds. `None` when either
    /// is zero or past its ceiling ([`MAX_SOURCES`], [`MAX_RUN_SECS`]).
    #[must_use]
    pub const fn new(sources: usize, secs: i64) -> Option<Self> {
        if sources == 0 || sources > MAX_SOURCES || secs <= 0 || secs > MAX_RUN_SECS {
            None
        } else {
            Some(Self { sources, secs })
        }
    }

    /// The most sources the run may gather.
    #[must_use]
    pub const fn sources(self) -> usize {
        self.sources
    }

    /// The most seconds the run may take.
    #[must_use]
    pub const fn secs(self) -> i64 {
        self.secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_bounds_and_the_ceilings_are_pinned() {
        assert_eq!(Bounds::standard().sources(), 8);
        assert_eq!(Bounds::standard().secs(), 900);
        assert_eq!(MAX_SOURCES, 32);
        assert_eq!(MAX_RUN_SECS, 3600);
    }

    #[test]
    fn bounds_are_never_zero_or_past_their_ceilings() {
        let ok = Bounds::new(1, 1).unwrap();
        assert_eq!((ok.sources(), ok.secs()), (1, 1));
        let top = Bounds::new(MAX_SOURCES, MAX_RUN_SECS).unwrap();
        assert_eq!((top.sources(), top.secs()), (32, 3600));
        for (sources, secs) in [(0, 60), (33, 60), (4, 0), (4, -1), (4, 3601)] {
            assert_eq!(Bounds::new(sources, secs), None, "{sources} {secs}");
        }
    }
}
