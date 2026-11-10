use kurbo::{BezPath, PathEl};

/// Seeds the jitter of one sketched element from its geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Seed(u64);

impl Seed {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    /// Hash the coordinate bits of every point of `path` into a seed with 64-bit FNV-1a, so a path always sketches alike.
    pub(crate) fn from_path(path: &BezPath) -> Self {
        let points = path.elements().iter().flat_map(|element| match *element {
            PathEl::MoveTo(point) | PathEl::LineTo(point) => vec![point],
            PathEl::QuadTo(control, end) => vec![control, end],
            PathEl::CurveTo(control1, control2, end) => vec![control1, control2, end],
            PathEl::ClosePath => Vec::new(),
        });
        let coordinate_bytes = points.flat_map(|point| {
            let x_bytes = point.x.to_bits().to_le_bytes();
            let y_bytes = point.y.to_bits().to_le_bytes();
            x_bytes.into_iter().chain(y_bytes)
        });
        let hash = Self::fnv1a(Self::FNV_OFFSET_BASIS, coordinate_bytes);
        Self(hash)
    }

    /// Continue hashing `salt` after this seed, deriving the seed of another stroke of the same geometry.
    pub(crate) fn salted(self, salt: &str) -> Self {
        let hash = Self::fnv1a(self.0, salt.bytes());
        Self(hash)
    }

    /// Fold `bytes` into `hash`: hash ← (hash ⊕ byte) · FNV prime, wrapping.
    fn fnv1a(hash: u64, bytes: impl Iterator<Item = u8>) -> u64 {
        bytes.fold(hash, |hash, byte| {
            let mixed_hash = hash ^ u64::from(byte);
            mixed_hash.wrapping_mul(Self::FNV_PRIME)
        })
    }
}

/// Generates the uniform jitter stream a seed starts, with splitmix64.
#[derive(Clone, Debug)]
pub(super) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;
    const UNIT_SCALE: f64 = 1.0 / (1u64 << 53) as f64;

    /// Start the stream `seed` determines.
    pub(super) fn new(seed: Seed) -> Self {
        Self { state: seed.0 }
    }

    /// Draw the next value in [0, 1): the top 53 bits of `next_u64` times 2⁻⁵³.
    pub(super) fn next_unit(&mut self) -> f64 {
        let top_53_bits = self.next_u64() >> 11;
        top_53_bits as f64 * Self::UNIT_SCALE
    }

    /// Draw the next 64 bits: z ← state += γ; z ← (z ⊕ z≫30)·m₁; z ← (z ⊕ z≫27)·m₂; z ⊕ z≫31.
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(Self::GOLDEN_GAMMA);
        let z = self.state;
        let z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}
