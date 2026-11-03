//! Seeded pseudo-random numbers (docs/architecture.md "Simulation model"): the
//! only source of randomness in the core. Hand-rolled PCG32 rather than a crate,
//! so determinism does not ride the dependency treadmill.

/// PCG32 (XSH RR, 64-bit state, fixed stream). Small, fast, and reproducible
/// from one `u64` seed, which is all a serve angle and an AI error need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
}

impl Pcg32 {
    const MULTIPLIER: u64 = 6_364_136_223_846_793_005;
    const INCREMENT: u64 = 1_442_695_040_888_963_407;

    /// A generator whose whole future is determined by `seed`.
    #[must_use]
    pub fn seed(seed: u64) -> Self {
        let mut rng = Self { state: 0 };
        rng.step();
        rng.state = rng.state.wrapping_add(seed);
        rng.step();
        rng
    }

    /// The next 32 random bits.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "PCG32 output is the low 32 bits by definition"
    )]
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.step();
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rotation = (old >> 59) as u32;
        xorshifted.rotate_right(rotation)
    }

    /// Uniform in `0.0..1.0`: the top 24 bits, so every value is exact in `f32`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "24 bits fit an f32 mantissa exactly"
    )]
    pub fn next_unit_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Uniform in `lo..hi`. Precondition: `lo <= hi`, both finite.
    pub fn next_f32_in(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_unit_f32()
    }

    fn step(&mut self) {
        self.state = self
            .state
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::INCREMENT);
    }
}

#[cfg(test)]
mod tests {
    use super::Pcg32;

    #[test]
    fn the_same_seed_reproduces_the_same_sequence() {
        let mut a = Pcg32::seed(42);
        let mut b = Pcg32::seed(42);
        let first: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
        let second: Vec<u32> = (0..8).map(|_| b.next_u32()).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Pcg32::seed(1);
        let mut b = Pcg32::seed(2);
        assert_ne!(a.next_u32(), b.next_u32());
    }

    #[test]
    fn unit_floats_stay_in_the_half_open_unit_interval_and_vary() {
        let mut rng = Pcg32::seed(7);
        let samples: Vec<f32> = (0..1_000).map(|_| rng.next_unit_f32()).collect();
        assert!(samples.iter().all(|x| (0.0..1.0).contains(x)));
        assert!(samples.iter().any(|&x| x < 0.25) && samples.iter().any(|&x| x > 0.75));
    }

    #[test]
    fn a_range_sample_lies_within_the_range() {
        let mut rng = Pcg32::seed(3);
        for _ in 0..1_000 {
            let x = rng.next_f32_in(-0.5, 0.5);
            assert!((-0.5..0.5).contains(&x), "{x}");
        }
    }
}
