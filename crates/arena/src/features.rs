use engine_core::{Board, ClearedLines, HEIGHT, PieceState, WIDTH};

pub const FEATURE_COUNT: usize = 10;

pub const FEATURE_NAMES: [&str; FEATURE_COUNT] = [
    "landing_height_x2",
    "eroded_piece_cells",
    "row_transitions",
    "column_transitions",
    "buried_holes",
    "cumulative_wells",
    "aggregate_height",
    "bumpiness",
    "max_height",
    "lines_cleared",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AfterstateAnalysis {
    pub(crate) features: [i32; FEATURE_COUNT],
    pub(crate) heights: [i32; WIDTH],
    pub(crate) holes: [i32; WIDTH],
}

/// Extracts the integer feature contract used by the linear teacher and the
/// first small neural scorer. The board must already contain the placement and
/// have completed line compaction.
pub fn extract_afterstate_features(
    board: &Board,
    placement: PieceState,
    cleared: ClearedLines,
) -> [i32; FEATURE_COUNT] {
    analyze_afterstate(board, placement, cleared).features
}

pub(crate) fn analyze_afterstate(
    board: &Board,
    placement: PieceState,
    cleared: ClearedLines,
) -> AfterstateAnalysis {
    let columns = column_masks(board);
    let (heights, holes) = metrics_from_columns(&columns);
    let aggregate_height = heights.iter().sum::<i32>();
    let bumpiness = heights
        .windows(2)
        .map(|pair| (pair[0] - pair[1]).abs())
        .sum();
    let max_height = heights.iter().copied().max().unwrap_or(0);

    AfterstateAnalysis {
        features: [
            landing_height_x2(placement),
            eroded_piece_cells(placement, cleared),
            row_transitions(board),
            column_transitions(&columns),
            holes.iter().sum(),
            cumulative_wells(&columns),
            aggregate_height,
            bumpiness,
            max_height,
            i32::from(cleared.count()),
        ],
        heights,
        holes,
    }
}

pub(crate) fn board_summary(board: &Board) -> [i32; 4] {
    let columns = column_masks(board);
    let (heights, holes) = metrics_from_columns(&columns);
    let aggregate_height = heights.iter().sum::<i32>();
    let max_height = heights.iter().copied().max().unwrap_or(0);
    let bumpiness = heights
        .windows(2)
        .map(|pair| (pair[0] - pair[1]).abs())
        .sum();
    [aggregate_height, max_height, holes.iter().sum(), bumpiness]
}

fn landing_height_x2(placement: PieceState) -> i32 {
    let cells = placement.cells();
    let min_y = cells.iter().map(|(_, y)| *y).min().unwrap_or(0);
    let max_y = cells.iter().map(|(_, y)| *y).max().unwrap_or(0);
    i32::from(min_y + max_y + 2)
}

fn eroded_piece_cells(placement: PieceState, cleared: ClearedLines) -> i32 {
    let cleared_rows = cleared.rows();
    let piece_cells = placement
        .cells()
        .iter()
        .filter(|(_, y)| u8::try_from(*y).is_ok_and(|row| cleared_rows.contains(&row)))
        .count();
    i32::from(cleared.count()) * i32::try_from(piece_cells).expect("tetromino has four cells")
}

pub(crate) fn column_heights_and_holes(board: &Board) -> ([i32; WIDTH], [i32; WIDTH]) {
    metrics_from_columns(&column_masks(board))
}

fn row_transitions(board: &Board) -> i32 {
    let internal_mask = (1_u16 << (WIDTH - 1)) - 1;
    board
        .rows()
        .iter()
        .map(|row| {
            let internal = (row ^ (row >> 1)) & internal_mask;
            internal.count_ones() + u32::from(row & 1 == 0) + u32::from(row >> (WIDTH - 1) == 0)
        })
        .sum::<u32>() as i32
}

fn column_transitions(columns: &[u64; WIDTH]) -> i32 {
    let internal_mask = (1_u64 << (HEIGHT - 1)) - 1;
    columns
        .iter()
        .map(|column| {
            let internal = (column ^ (column >> 1)) & internal_mask;
            internal.count_ones()
                + u32::from(column & 1 == 0)
                + u32::from(column >> (HEIGHT - 1) == 0)
        })
        .sum::<u32>() as i32
}

fn column_masks(board: &Board) -> [u64; WIDTH] {
    let mut columns = [0_u64; WIDTH];
    for (y, row) in board.rows().iter().copied().enumerate() {
        for (x, column) in columns.iter_mut().enumerate() {
            *column |= u64::from((row >> x) & 1) << y;
        }
    }
    columns
}

fn metrics_from_columns(columns: &[u64; WIDTH]) -> ([i32; WIDTH], [i32; WIDTH]) {
    let mut heights = [0_i32; WIDTH];
    let mut holes = [0_i32; WIDTH];
    for (index, column) in columns.iter().copied().enumerate() {
        let height = if column == 0 {
            0
        } else {
            u64::BITS - column.leading_zeros()
        };
        heights[index] = i32::try_from(height).expect("board height fits i32");
        holes[index] = heights[index]
            - i32::try_from(column.count_ones()).expect("column population fits i32");
    }
    (heights, holes)
}

fn cumulative_wells(columns: &[u64; WIDTH]) -> i32 {
    let board_mask = (1_u64 << HEIGHT) - 1;
    let mut total = 0;
    for x in 0..WIDTH {
        let left = if x == 0 { board_mask } else { columns[x - 1] };
        let right = if x + 1 == WIDTH {
            board_mask
        } else {
            columns[x + 1]
        };
        let mut wells = !columns[x] & left & right & board_mask;
        while wells != 0 {
            total += i32::try_from(wells.count_ones()).expect("well population fits i32");
            wells &= wells << 1;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::{Orientation, PieceKind};

    #[test]
    fn empty_board_features_have_only_boundary_transitions() {
        let board = Board::empty();
        let piece = PieceState::new(PieceKind::O, Orientation::Spawn, 3, -1);
        let features = extract_afterstate_features(&board, piece, ClearedLines::default());

        assert_eq!(features[2], i32::try_from(HEIGHT * 2).unwrap());
        assert_eq!(features[3], i32::try_from(WIDTH * 2).unwrap());
        assert_eq!(features[4], 0);
        assert_eq!(features[5], 0);
        assert_eq!(features[6], 0);
    }

    #[test]
    fn eroded_piece_cells_multiplies_cells_by_cleared_lines() {
        let mut board = Board::from_rows([0; HEIGHT]).unwrap();
        for x in 0..8 {
            board.set_cell(x, 0, true).unwrap();
        }
        let piece = PieceState::new(PieceKind::O, Orientation::Spawn, 7, -1);
        let lock = board.lock(piece).unwrap();
        let features = extract_afterstate_features(&board, piece, lock.cleared);

        assert_eq!(lock.cleared.count(), 1);
        assert_eq!(features[1], 2);
        assert_eq!(features[9], 1);
    }

    #[test]
    fn bitboard_features_match_naive_cell_scans() {
        let piece = PieceState::new(PieceKind::T, Orientation::Right, 3, 8);
        let mut state = 0x243f_6a88_u32;
        for case in 0..128 {
            let mut rows = [0_u16; HEIGHT];
            for row in &mut rows {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *row = ((state >> 16) as u16) & 0x03ff;
                if *row == 0x03ff {
                    *row ^= 1_u16 << (case % WIDTH);
                }
            }
            let board = Board::from_rows(rows).unwrap();
            let actual = extract_afterstate_features(&board, piece, ClearedLines::default());
            assert_eq!(actual[2], naive_row_transitions(&board));
            assert_eq!(actual[3], naive_column_transitions(&board));
            assert_eq!(actual[4], naive_holes(&board));
            assert_eq!(actual[5], naive_wells(&board));
        }
    }

    fn naive_occupied(board: &Board, x: usize, y: usize) -> bool {
        board.is_occupied(x, y).unwrap()
    }

    fn naive_row_transitions(board: &Board) -> i32 {
        let mut transitions = 0;
        for y in 0..HEIGHT {
            let mut previous = true;
            for x in 0..WIDTH {
                let cell = naive_occupied(board, x, y);
                transitions += i32::from(cell != previous);
                previous = cell;
            }
            transitions += i32::from(!previous);
        }
        transitions
    }

    fn naive_column_transitions(board: &Board) -> i32 {
        let mut transitions = 0;
        for x in 0..WIDTH {
            let mut previous = true;
            for y in 0..HEIGHT {
                let cell = naive_occupied(board, x, y);
                transitions += i32::from(cell != previous);
                previous = cell;
            }
            transitions += i32::from(!previous);
        }
        transitions
    }

    fn naive_holes(board: &Board) -> i32 {
        let mut holes = 0;
        for x in 0..WIDTH {
            let height = (0..HEIGHT)
                .rev()
                .find(|y| naive_occupied(board, x, *y))
                .map_or(0, |y| y + 1);
            holes += (0..height)
                .filter(|y| !naive_occupied(board, x, *y))
                .count() as i32;
        }
        holes
    }

    fn naive_wells(board: &Board) -> i32 {
        let mut total = 0;
        for x in 0..WIDTH {
            let mut depth = 0;
            for y in 0..HEIGHT {
                let cell = naive_occupied(board, x, y);
                let left = x == 0 || naive_occupied(board, x - 1, y);
                let right = x + 1 == WIDTH || naive_occupied(board, x + 1, y);
                if !cell && left && right {
                    depth += 1;
                    total += depth;
                } else {
                    depth = 0;
                }
            }
        }
        total
    }
}
