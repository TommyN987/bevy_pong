//! The core's seed for a real launch (plan 4.5): fresh entropy each launch, so
//! matches differ, from std's `RandomState` rather than an extra crate.
//! Tests seed the core with constants instead.

use std::hash::{BuildHasher, RandomState};

/// A seed that differs from launch to launch.
///
/// `RandomState` draws its keys from the operating system's randomness once
/// per thread and derives every later key from them, so two launches seed
/// differently and two calls in one launch do too. Hashing nothing with
/// those keys turns them into one `u64`.
#[must_use]
pub fn from_entropy() -> u64 {
    RandomState::new().hash_one(())
}

#[cfg(test)]
mod tests {
    use super::from_entropy;

    #[test]
    fn two_seeds_differ() {
        assert_ne!(from_entropy(), from_entropy());
    }
}
