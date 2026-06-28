//! A tiny deterministic pseudo-random number generator.
//!
//! Solitaire only needs to shuffle a 52-card deck, so it carries its own RNG
//! rather than pulling in the `rand` crate. Seeding it explicitly makes the
//! deal a pure function of the seed — which is exactly what the snapshot tests
//! and the scripted demo rely on for byte-stable output across machines.
//!
//! The algorithm is SplitMix64 (the same mixer Rust's `rand` uses to seed
//! larger generators): a single 64-bit state advanced by a fixed increment and
//! run through a strong avalanche. More than good enough for a card shuffle.

pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a generator from a 64-bit seed.
    pub fn seeded(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// The next 64-bit value (SplitMix64).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniformly-distributed value in `[0, bound)`. Uses Lemire's
    /// multiply-shift reduction, which is unbiased enough for a shuffle and
    /// needs no rejection loop in practice.
    pub fn below(&mut self, bound: usize) -> usize {
        if bound <= 1 {
            return 0;
        }
        let product = (self.next_u64() as u128) * (bound as u128);
        (product >> 64) as usize
    }

    /// In-place Fisher–Yates shuffle. Deterministic for a given seed.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::seeded(42);
        let mut b = Rng::seeded(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::seeded(1);
        let mut b = Rng::seeded(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut v: Vec<i32> = (0..52).collect();
        let original = v.clone();
        Rng::seeded(7).shuffle(&mut v);
        assert_ne!(v, original, "a 52-element shuffle should reorder");
        let mut sorted = v.clone();
        sorted.sort();
        assert_eq!(sorted, original, "shuffle must preserve the multiset");
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = Rng::seeded(123);
        for _ in 0..1000 {
            assert!(rng.below(10) < 10);
        }
        assert_eq!(rng.below(1), 0);
        assert_eq!(rng.below(0), 0);
    }
}
