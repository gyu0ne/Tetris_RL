/// The seven tetromino kinds.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum PieceKind {
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}

impl PieceKind {
    /// Canonical engine order. A rules profile may pass a different order to
    /// the bag generator without changing the core.
    pub const ALL: [Self; 7] = [
        Self::I,
        Self::J,
        Self::L,
        Self::O,
        Self::S,
        Self::T,
        Self::Z,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    /// Occupied cells in an SRS-compatible 4×4 local coordinate box.
    /// Coordinates use positive y upward.
    pub const fn cells(self, orientation: Orientation) -> [(i16, i16); 4] {
        match (self, orientation) {
            (Self::I, Orientation::Spawn) => [(0, 2), (1, 2), (2, 2), (3, 2)],
            (Self::I, Orientation::Right) => [(2, 0), (2, 1), (2, 2), (2, 3)],
            (Self::I, Orientation::Reverse) => [(0, 1), (1, 1), (2, 1), (3, 1)],
            (Self::I, Orientation::Left) => [(1, 0), (1, 1), (1, 2), (1, 3)],

            (Self::J, Orientation::Spawn) => [(0, 2), (0, 1), (1, 1), (2, 1)],
            (Self::J, Orientation::Right) => [(1, 2), (2, 2), (1, 1), (1, 0)],
            (Self::J, Orientation::Reverse) => [(0, 1), (1, 1), (2, 1), (2, 0)],
            (Self::J, Orientation::Left) => [(1, 2), (1, 1), (0, 0), (1, 0)],

            (Self::L, Orientation::Spawn) => [(2, 2), (0, 1), (1, 1), (2, 1)],
            (Self::L, Orientation::Right) => [(1, 2), (1, 1), (1, 0), (2, 0)],
            (Self::L, Orientation::Reverse) => [(0, 1), (1, 1), (2, 1), (0, 0)],
            (Self::L, Orientation::Left) => [(0, 2), (1, 2), (1, 1), (1, 0)],

            (Self::O, _) => [(1, 2), (2, 2), (1, 1), (2, 1)],

            (Self::S, Orientation::Spawn) => [(1, 2), (2, 2), (0, 1), (1, 1)],
            (Self::S, Orientation::Right) => [(1, 2), (1, 1), (2, 1), (2, 0)],
            (Self::S, Orientation::Reverse) => [(0, 1), (1, 1), (1, 0), (2, 0)],
            (Self::S, Orientation::Left) => [(0, 2), (0, 1), (1, 1), (1, 0)],

            (Self::T, Orientation::Spawn) => [(0, 1), (1, 1), (2, 1), (1, 2)],
            (Self::T, Orientation::Right) => [(1, 0), (1, 1), (1, 2), (2, 1)],
            (Self::T, Orientation::Reverse) => [(0, 1), (1, 1), (2, 1), (1, 0)],
            (Self::T, Orientation::Left) => [(1, 0), (1, 1), (1, 2), (0, 1)],

            (Self::Z, Orientation::Spawn) => [(0, 2), (1, 2), (1, 1), (2, 1)],
            (Self::Z, Orientation::Right) => [(2, 2), (1, 1), (2, 1), (1, 0)],
            (Self::Z, Orientation::Reverse) => [(0, 0), (1, 0), (1, 1), (2, 1)],
            (Self::Z, Orientation::Left) => [(1, 2), (0, 1), (1, 1), (0, 0)],
        }
    }
}

/// SRS orientation names.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Orientation {
    Spawn,
    Right,
    Reverse,
    Left,
}

impl Orientation {
    pub const ALL: [Self; 4] = [Self::Spawn, Self::Right, Self::Reverse, Self::Left];

    pub const fn clockwise(self) -> Self {
        match self {
            Self::Spawn => Self::Right,
            Self::Right => Self::Reverse,
            Self::Reverse => Self::Left,
            Self::Left => Self::Spawn,
        }
    }

    pub const fn counterclockwise(self) -> Self {
        match self {
            Self::Spawn => Self::Left,
            Self::Right => Self::Spawn,
            Self::Reverse => Self::Right,
            Self::Left => Self::Reverse,
        }
    }

    pub const fn half(self) -> Self {
        match self {
            Self::Spawn => Self::Reverse,
            Self::Right => Self::Left,
            Self::Reverse => Self::Spawn,
            Self::Left => Self::Right,
        }
    }
}

/// A tetromino state. `x` and `y` locate the bottom-left of its 4×4 SRS box.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PieceState {
    pub kind: PieceKind,
    pub orientation: Orientation,
    pub x: i16,
    pub y: i16,
}

impl PieceState {
    pub const fn new(kind: PieceKind, orientation: Orientation, x: i16, y: i16) -> Self {
        Self {
            kind,
            orientation,
            x,
            y,
        }
    }

    pub fn cells(self) -> [(i16, i16); 4] {
        PieceKind::cells(self.kind, self.orientation)
            .map(|(local_x, local_y)| (self.x + local_x, self.y + local_y))
    }

    pub const fn translated(self, dx: i16, dy: i16) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }

    pub const fn with_orientation(self, orientation: Orientation) -> Self {
        Self {
            orientation,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Orientation, PieceKind, PieceState};
    use std::collections::BTreeSet;

    #[test]
    fn every_orientation_has_four_unique_cells() {
        for kind in PieceKind::ALL {
            for orientation in Orientation::ALL {
                let cells = BTreeSet::from(kind.cells(orientation));
                assert_eq!(cells.len(), 4, "{kind:?} {orientation:?}");
                assert!(
                    cells
                        .iter()
                        .all(|&(x, y)| (0..4).contains(&x) && (0..4).contains(&y))
                );
            }
        }
    }

    #[test]
    fn four_clockwise_rotations_return_to_spawn() {
        let mut orientation = Orientation::Spawn;
        for _ in 0..4 {
            orientation = orientation.clockwise();
        }
        assert_eq!(orientation, Orientation::Spawn);
    }

    #[test]
    fn global_cells_include_piece_origin() {
        let piece = PieceState::new(PieceKind::T, Orientation::Spawn, 3, 10);
        assert_eq!(piece.cells(), [(3, 11), (4, 11), (5, 11), (4, 12)]);
    }
}
