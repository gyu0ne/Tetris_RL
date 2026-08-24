use crate::PieceState;
use std::fmt;

pub const WIDTH: usize = 10;
pub const HEIGHT: usize = 40;
pub const VISIBLE_HEIGHT: usize = 20;
const FULL_ROW: u16 = (1_u16 << WIDTH) - 1;

/// A row-oriented 10×40 bitboard. Row zero is the floor and positive y is up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Board {
    rows: [u16; HEIGHT],
}

impl Default for Board {
    fn default() -> Self {
        Self::empty()
    }
}

impl Board {
    pub const fn empty() -> Self {
        Self { rows: [0; HEIGHT] }
    }

    pub fn from_rows(rows: [u16; HEIGHT]) -> Result<Self, BoardError> {
        if let Some((y, row)) = rows
            .iter()
            .copied()
            .enumerate()
            .find(|(_, row)| row & !FULL_ROW != 0)
        {
            return Err(BoardError::InvalidRow { y, row });
        }
        Ok(Self { rows })
    }

    pub const fn rows(&self) -> &[u16; HEIGHT] {
        &self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(|row| *row == 0)
    }

    pub fn row(&self, y: usize) -> Option<u16> {
        self.rows.get(y).copied()
    }

    pub fn is_occupied(&self, x: usize, y: usize) -> Option<bool> {
        self.rows
            .get(y)
            .filter(|_| x < WIDTH)
            .map(|row| row & (1_u16 << x) != 0)
    }

    pub fn set_cell(&mut self, x: usize, y: usize, occupied: bool) -> Result<(), BoardError> {
        if x >= WIDTH || y >= HEIGHT {
            return Err(BoardError::CellOutOfBounds { x, y });
        }
        let mask = 1_u16 << x;
        if occupied {
            self.rows[y] |= mask;
        } else {
            self.rows[y] &= !mask;
        }
        Ok(())
    }

    pub fn collides(&self, piece: PieceState) -> bool {
        piece.cells().into_iter().any(|(x, y)| {
            if x < 0 || x >= WIDTH as i16 || y < 0 || y >= HEIGHT as i16 {
                return true;
            }
            self.rows[y as usize] & (1_u16 << x) != 0
        })
    }

    pub fn lock(&mut self, piece: PieceState) -> Result<LockResult, BoardError> {
        if self.collides(piece) {
            return Err(BoardError::PieceCollision(piece));
        }

        let cells = piece.cells();
        let hidden_cells = cells
            .iter()
            .filter(|(_, y)| *y >= VISIBLE_HEIGHT as i16)
            .count();
        let visibility = match hidden_cells {
            0 => LockVisibility::Visible,
            4 => LockVisibility::FullyHidden,
            _ => LockVisibility::PartiallyHidden,
        };

        for (x, y) in cells {
            self.rows[y as usize] |= 1_u16 << x;
        }

        let cleared = self.clear_full_lines();
        Ok(LockResult {
            cleared,
            perfect_clear: self.is_empty(),
            visibility,
        })
    }

    pub fn stack_height(&self) -> usize {
        self.rows
            .iter()
            .rposition(|row| *row != 0)
            .map_or(0, |y| y + 1)
    }

    pub fn occupied_cells(&self) -> u32 {
        self.rows.iter().map(|row| row.count_ones()).sum()
    }

    /// Stable FNV-1a checksum for replay checkpoints. This is not a cryptographic hash.
    pub fn checksum(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for row in self.rows {
            for byte in row.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }

    fn clear_full_lines(&mut self) -> ClearedLines {
        let mut cleared = ClearedLines::default();
        let mut write_y = 0;

        for read_y in 0..HEIGHT {
            if self.rows[read_y] == FULL_ROW {
                cleared.push(read_y as u8);
            } else {
                self.rows[write_y] = self.rows[read_y];
                write_y += 1;
            }
        }

        self.rows[write_y..].fill(0);
        cleared
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClearedLines {
    count: u8,
    rows: [u8; 4],
}

impl ClearedLines {
    pub const fn count(self) -> u8 {
        self.count
    }

    pub fn rows(&self) -> &[u8] {
        &self.rows[..usize::from(self.count)]
    }

    fn push(&mut self, row: u8) {
        debug_assert!(usize::from(self.count) < self.rows.len());
        self.rows[usize::from(self.count)] = row;
        self.count += 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LockResult {
    pub cleared: ClearedLines,
    pub perfect_clear: bool,
    pub visibility: LockVisibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockVisibility {
    Visible,
    PartiallyHidden,
    FullyHidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardError {
    InvalidRow { y: usize, row: u16 },
    CellOutOfBounds { x: usize, y: usize },
    PieceCollision(PieceState),
}

impl fmt::Display for BoardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRow { y, row } => {
                write!(formatter, "row {y} has out-of-width bits: {row:#06x}")
            }
            Self::CellOutOfBounds { x, y } => {
                write!(formatter, "cell ({x}, {y}) is outside the board")
            }
            Self::PieceCollision(piece) => write!(formatter, "piece collides at {piece:?}"),
        }
    }
}

impl std::error::Error for BoardError {}

#[cfg(test)]
mod tests {
    use super::{Board, BoardError, FULL_ROW, HEIGHT, LockVisibility};
    use crate::{Orientation, PieceKind, PieceState};

    #[test]
    fn rejects_bits_outside_width() {
        let mut rows = [0; HEIGHT];
        rows[3] = 1 << 10;
        assert_eq!(
            Board::from_rows(rows),
            Err(BoardError::InvalidRow { y: 3, row: 1 << 10 })
        );
    }

    #[test]
    fn locking_piece_updates_cells_and_checksum() {
        let mut board = Board::empty();
        let before = board.checksum();
        let piece = PieceState::new(PieceKind::O, Orientation::Spawn, 3, -1);
        let result = board.lock(piece).expect("O piece should fit on floor");
        assert_eq!(result.cleared.count(), 0);
        assert!(!result.perfect_clear);
        assert_eq!(result.visibility, LockVisibility::Visible);
        assert_eq!(board.occupied_cells(), 4);
        assert_ne!(board.checksum(), before);
    }

    #[test]
    fn full_rows_are_compacted_in_order() {
        let mut rows = [0; HEIGHT];
        rows[0] = FULL_ROW;
        rows[1] = 0b00_0000_0001;
        rows[2] = FULL_ROW;
        rows[3] = 0b00_0000_0010;
        let mut board = Board::from_rows(rows).expect("valid rows");

        let piece = PieceState::new(PieceKind::O, Orientation::Spawn, 3, 3);
        let result = board.lock(piece).expect("piece should fit");

        assert_eq!(result.cleared.rows(), &[0, 2]);
        assert_eq!(board.row(0), Some(0b00_0000_0001));
        assert_eq!(board.row(1), Some(0b00_0000_0010));
        assert_eq!(board.occupied_cells(), 6);
    }

    #[test]
    fn floor_and_walls_collide() {
        let board = Board::empty();
        assert!(board.collides(PieceState::new(PieceKind::T, Orientation::Spawn, -1, -1)));
        assert!(board.collides(PieceState::new(PieceKind::I, Orientation::Spawn, 7, 0)));
        assert!(!board.collides(PieceState::new(PieceKind::I, Orientation::Spawn, 6, 0)));
    }

    #[test]
    fn clearing_the_last_cells_reports_perfect_clear() {
        let mut rows = [0; HEIGHT];
        rows[0] = FULL_ROW & !0b00_0111_1000;
        let mut board = Board::from_rows(rows).expect("valid almost-full row");
        let piece = PieceState::new(PieceKind::I, Orientation::Reverse, 3, -1);

        let result = board.lock(piece).expect("I piece completes the row");
        assert_eq!(result.cleared.rows(), &[0]);
        assert!(result.perfect_clear);
        assert!(board.is_empty());
    }

    #[test]
    fn lock_visibility_distinguishes_partial_and_full_hidden_placement() {
        let mut partial_board = Board::empty();
        let partial = partial_board
            .lock(PieceState::new(PieceKind::O, Orientation::Spawn, 3, 18))
            .expect("partially hidden O fits");
        assert_eq!(partial.visibility, LockVisibility::PartiallyHidden);

        let mut hidden_board = Board::empty();
        let hidden = hidden_board
            .lock(PieceState::new(PieceKind::O, Orientation::Spawn, 3, 19))
            .expect("fully hidden O fits");
        assert_eq!(hidden.visibility, LockVisibility::FullyHidden);
    }
}
