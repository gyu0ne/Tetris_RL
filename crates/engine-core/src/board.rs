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
    garbage_rows: [u16; HEIGHT],
}

impl Default for Board {
    fn default() -> Self {
        Self::empty()
    }
}

impl Board {
    pub const fn empty() -> Self {
        Self {
            rows: [0; HEIGHT],
            garbage_rows: [0; HEIGHT],
        }
    }

    pub fn from_rows(rows: [u16; HEIGHT]) -> Result<Self, BoardError> {
        Self::from_layers(rows, [0; HEIGHT])
    }

    pub fn from_layers(
        rows: [u16; HEIGHT],
        garbage_rows: [u16; HEIGHT],
    ) -> Result<Self, BoardError> {
        if let Some((y, row)) = rows
            .iter()
            .copied()
            .enumerate()
            .find(|(_, row)| row & !FULL_ROW != 0)
        {
            return Err(BoardError::InvalidRow { y, row });
        }
        if let Some((y, garbage, occupied)) = garbage_rows
            .iter()
            .copied()
            .zip(rows.iter().copied())
            .enumerate()
            .find_map(|(y, (garbage, occupied))| {
                (garbage & !occupied != 0).then_some((y, garbage, occupied))
            })
        {
            return Err(BoardError::GarbageOutsideOccupancy {
                y,
                garbage,
                occupied,
            });
        }
        Ok(Self { rows, garbage_rows })
    }

    pub const fn rows(&self) -> &[u16; HEIGHT] {
        &self.rows
    }

    pub const fn garbage_rows(&self) -> &[u16; HEIGHT] {
        &self.garbage_rows
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

    pub fn is_garbage(&self, x: usize, y: usize) -> Option<bool> {
        self.garbage_rows
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
            self.garbage_rows[y] &= !mask;
        }
        Ok(())
    }

    pub fn set_garbage_cell(
        &mut self,
        x: usize,
        y: usize,
        garbage: bool,
    ) -> Result<(), BoardError> {
        if x >= WIDTH || y >= HEIGHT {
            return Err(BoardError::CellOutOfBounds { x, y });
        }
        let mask = 1_u16 << x;
        if garbage {
            self.rows[y] |= mask;
            self.garbage_rows[y] |= mask;
        } else {
            self.garbage_rows[y] &= !mask;
        }
        Ok(())
    }

    /// Pushes one garbage row from the floor, shifting every existing row up.
    /// The return value reports only loss beyond the 40-row storage buffer;
    /// mode-specific top-out policy remains outside this primitive.
    pub fn push_garbage_line(
        &mut self,
        hole_column: usize,
    ) -> Result<GarbagePushResult, BoardError> {
        if hole_column >= WIDTH {
            return Err(BoardError::GarbageHoleOutOfBounds(hole_column));
        }

        let overflowed_buffer = self.rows[HEIGHT - 1] != 0;
        self.rows.copy_within(0..HEIGHT - 1, 1);
        self.garbage_rows.copy_within(0..HEIGHT - 1, 1);
        let garbage = FULL_ROW & !(1_u16 << hole_column);
        self.rows[0] = garbage;
        self.garbage_rows[0] = garbage;

        Ok(GarbagePushResult { overflowed_buffer })
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

        let (cleared, cleared_garbage) = self.clear_full_lines();
        Ok(LockResult {
            cleared,
            cleared_garbage,
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

    /// Current-client `AreWeToppedYet` checks whether the 40-row buffer's
    /// ceiling row is completely filled before an instant garbage rise.
    pub const fn buffer_ceiling_full(&self) -> bool {
        self.rows[HEIGHT - 1] == FULL_ROW
    }

    pub fn occupied_cells(&self) -> u32 {
        self.rows.iter().map(|row| row.count_ones()).sum()
    }

    /// Stable FNV-1a checksum for replay checkpoints. This is not a cryptographic hash.
    pub fn checksum(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for (row, garbage) in self.rows.into_iter().zip(self.garbage_rows) {
            for byte in row.to_le_bytes().into_iter().chain(garbage.to_le_bytes()) {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }

    fn clear_full_lines(&mut self) -> (ClearedLines, bool) {
        let mut cleared = ClearedLines::default();
        let mut cleared_garbage = false;
        let mut write_y = 0;

        for read_y in 0..HEIGHT {
            if self.rows[read_y] == FULL_ROW {
                cleared.push(read_y as u8);
                cleared_garbage |= self.garbage_rows[read_y] != 0;
            } else {
                self.rows[write_y] = self.rows[read_y];
                self.garbage_rows[write_y] = self.garbage_rows[read_y];
                write_y += 1;
            }
        }

        self.rows[write_y..].fill(0);
        self.garbage_rows[write_y..].fill(0);
        (cleared, cleared_garbage)
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
    pub cleared_garbage: bool,
    pub perfect_clear: bool,
    pub visibility: LockVisibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GarbagePushResult {
    pub overflowed_buffer: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockVisibility {
    Visible,
    PartiallyHidden,
    FullyHidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardError {
    InvalidRow {
        y: usize,
        row: u16,
    },
    GarbageOutsideOccupancy {
        y: usize,
        garbage: u16,
        occupied: u16,
    },
    CellOutOfBounds {
        x: usize,
        y: usize,
    },
    GarbageHoleOutOfBounds(usize),
    PieceCollision(PieceState),
}

impl fmt::Display for BoardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRow { y, row } => {
                write!(formatter, "row {y} has out-of-width bits: {row:#06x}")
            }
            Self::GarbageOutsideOccupancy {
                y,
                garbage,
                occupied,
            } => write!(
                formatter,
                "garbage row {y} ({garbage:#06x}) is not a subset of occupancy ({occupied:#06x})"
            ),
            Self::CellOutOfBounds { x, y } => {
                write!(formatter, "cell ({x}, {y}) is outside the board")
            }
            Self::GarbageHoleOutOfBounds(column) => {
                write!(
                    formatter,
                    "garbage hole column {column} is outside the board"
                )
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
    fn rejects_garbage_bits_without_occupancy() {
        let rows = [0; HEIGHT];
        let mut garbage_rows = [0; HEIGHT];
        garbage_rows[2] = 1 << 4;

        assert_eq!(
            Board::from_layers(rows, garbage_rows),
            Err(BoardError::GarbageOutsideOccupancy {
                y: 2,
                garbage: 1 << 4,
                occupied: 0,
            })
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
    fn garbage_push_shifts_both_layers_and_marks_the_new_row() {
        let mut board = Board::empty();
        board.set_cell(2, 0, true).expect("valid cell");

        let pushed = board.push_garbage_line(4).expect("valid hole");

        assert!(!pushed.overflowed_buffer);
        assert_eq!(board.row(0), Some(FULL_ROW & !(1 << 4)));
        assert_eq!(board.garbage_rows()[0], FULL_ROW & !(1 << 4));
        assert_eq!(board.row(1), Some(1 << 2));
        assert_eq!(board.garbage_rows()[1], 0);
        assert_eq!(board.is_garbage(4, 0), Some(false));
        assert_eq!(board.is_garbage(3, 0), Some(true));
    }

    #[test]
    fn garbage_push_reports_a_filled_buffer_ceiling() {
        let mut rows = [0; HEIGHT];
        rows[HEIGHT - 1] = FULL_ROW;
        let mut board = Board::from_rows(rows).expect("valid full ceiling");

        let pushed = board.push_garbage_line(4).expect("valid hole");

        assert!(pushed.overflowed_buffer);
        assert!(!board.buffer_ceiling_full());
    }

    #[test]
    fn clearing_a_garbage_row_reports_provenance() {
        let mut rows = [0; HEIGHT];
        rows[0] = FULL_ROW & !0b00_0111_1000;
        let garbage_rows = rows;
        let mut board = Board::from_layers(rows, garbage_rows).expect("valid garbage layer");
        let piece = PieceState::new(PieceKind::I, Orientation::Reverse, 3, -1);

        let result = board.lock(piece).expect("I piece completes garbage row");

        assert_eq!(result.cleared.rows(), &[0]);
        assert!(result.cleared_garbage);
        assert!(result.perfect_clear);
        assert!(board.garbage_rows().iter().all(|row| *row == 0));
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
