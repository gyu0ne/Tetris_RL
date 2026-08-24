use crate::PieceKind;
use std::fmt;

const MINSTD_MODULUS: u64 = 2_147_483_647;
const MINSTD_MULTIPLIER: u64 = 16_807;

/// Park-Miller MINSTD with explicit non-zero seed normalization.
///
/// The algorithm is available for versioned profiles, but using it here does
/// not assert that it is the current TETR.IO RNG.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinStd {
    state: u32,
}

impl MinStd {
    pub fn new(seed: u64) -> Self {
        let state = (seed % (MINSTD_MODULUS - 1) + 1) as u32;
        Self { state }
    }

    pub const fn state(self) -> u32 {
        self.state
    }

    pub fn next_u31(&mut self) -> u32 {
        self.state = (u64::from(self.state) * MINSTD_MULTIPLIER % MINSTD_MODULUS) as u32;
        self.state
    }

    fn index(&mut self, upper_exclusive: usize) -> usize {
        debug_assert!(upper_exclusive > 0);
        let scaled = u64::from(self.next_u31()) * upper_exclusive as u64;
        (scaled / MINSTD_MODULUS) as usize
    }
}

/// Deterministic 7-bag whose pre-shuffle order is supplied by a rules profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SevenBag {
    rng: MinStd,
    base_order: [PieceKind; 7],
    bag: [PieceKind; 7],
    cursor: usize,
}

impl SevenBag {
    pub fn new(seed: u64) -> Self {
        Self::with_order(seed, PieceKind::ALL).expect("canonical piece order is valid")
    }

    pub fn with_order(seed: u64, order: [PieceKind; 7]) -> Result<Self, BagOrderError> {
        validate_order(order)?;
        Ok(Self {
            rng: MinStd::new(seed),
            base_order: order,
            bag: order,
            cursor: 7,
        })
    }

    pub const fn rng_state(&self) -> u32 {
        self.rng.state()
    }

    pub const fn base_order(&self) -> &[PieceKind; 7] {
        &self.base_order
    }

    pub fn next_piece(&mut self) -> PieceKind {
        if self.cursor == self.bag.len() {
            self.refill();
        }
        let piece = self.bag[self.cursor];
        self.cursor += 1;
        piece
    }

    fn refill(&mut self) {
        self.bag = self.base_order;
        for index in (1..self.bag.len()).rev() {
            let swap_with = self.rng.index(index + 1);
            self.bag.swap(index, swap_with);
        }
        self.cursor = 0;
    }
}

fn validate_order(order: [PieceKind; 7]) -> Result<(), BagOrderError> {
    let mut seen = 0_u8;
    for piece in order {
        let bit = 1_u8 << piece.index();
        if seen & bit != 0 {
            return Err(BagOrderError::Duplicate(piece));
        }
        seen |= bit;
    }
    if seen != 0b0111_1111 {
        return Err(BagOrderError::MissingPiece);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BagOrderError {
    Duplicate(PieceKind),
    MissingPiece,
}

impl fmt::Display for BagOrderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(piece) => write!(formatter, "bag order contains duplicate {piece:?}"),
            Self::MissingPiece => write!(formatter, "bag order does not contain all seven pieces"),
        }
    }
}

impl std::error::Error for BagOrderError {}

#[cfg(test)]
mod tests {
    use super::{BagOrderError, MinStd, SevenBag};
    use crate::PieceKind;
    use std::collections::BTreeSet;

    #[test]
    fn minstd_is_reproducible_and_nonzero() {
        let mut first = MinStd::new(0);
        let mut second = MinStd::new(0);
        for _ in 0..100 {
            assert_eq!(first.next_u31(), second.next_u31());
            assert_ne!(first.state(), 0);
        }
    }

    #[test]
    fn each_chunk_is_a_permutation() {
        let mut bag = SevenBag::new(42);
        for _ in 0..32 {
            let chunk = (0..7).map(|_| bag.next_piece()).collect::<BTreeSet<_>>();
            assert_eq!(chunk, BTreeSet::from(PieceKind::ALL));
        }
    }

    #[test]
    fn same_seed_has_same_sequence() {
        let mut first = SevenBag::new(7);
        let mut second = SevenBag::new(7);
        let left = (0..100).map(|_| first.next_piece()).collect::<Vec<_>>();
        let right = (0..100).map(|_| second.next_piece()).collect::<Vec<_>>();
        assert_eq!(left, right);
    }

    #[test]
    fn duplicate_order_is_rejected() {
        let invalid = [PieceKind::I; 7];
        assert_eq!(
            SevenBag::with_order(1, invalid),
            Err(BagOrderError::Duplicate(PieceKind::I))
        );
    }
}
