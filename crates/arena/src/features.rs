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

/// Extracts the integer feature contract used by the linear teacher and the
/// first small neural scorer. The board must already contain the placement and
/// have completed line compaction.
pub fn extract_afterstate_features(
    board: &Board,
    placement: PieceState,
    cleared: ClearedLines,
) -> [i32; FEATURE_COUNT] {
    let heights = column_heights(board);
    let aggregate_height = heights.iter().sum::<i32>();
    let bumpiness = heights
        .windows(2)
        .map(|pair| (pair[0] - pair[1]).abs())
        .sum();
    let max_height = heights.iter().copied().max().unwrap_or(0);

    [
        landing_height_x2(placement),
        eroded_piece_cells(placement, cleared),
        row_transitions(board),
        column_transitions(board),
        buried_holes(board, &heights),
        cumulative_wells(board),
        aggregate_height,
        bumpiness,
        max_height,
        i32::from(cleared.count()),
    ]
}

pub(crate) fn board_summary(board: &Board) -> [i32; 4] {
    let heights = column_heights(board);
    let aggregate_height = heights.iter().sum::<i32>();
    let max_height = heights.iter().copied().max().unwrap_or(0);
    let holes = buried_holes(board, &heights);
    let bumpiness = heights
        .windows(2)
        .map(|pair| (pair[0] - pair[1]).abs())
        .sum();
    [aggregate_height, max_height, holes, bumpiness]
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

fn column_heights(board: &Board) -> [i32; WIDTH] {
    let mut heights = [0; WIDTH];
    for (x, height) in heights.iter_mut().enumerate() {
        for y in (0..HEIGHT).rev() {
            if occupied(board, x, y) {
                *height = i32::try_from(y + 1).expect("board height fits i32");
                break;
            }
        }
    }
    heights
}

fn row_transitions(board: &Board) -> i32 {
    let mut transitions = 0;
    for y in 0..HEIGHT {
        let mut previous = true;
        for x in 0..WIDTH {
            let cell = occupied(board, x, y);
            transitions += i32::from(cell != previous);
            previous = cell;
        }
        transitions += i32::from(!previous);
    }
    transitions
}

fn column_transitions(board: &Board) -> i32 {
    let mut transitions = 0;
    for x in 0..WIDTH {
        let mut previous = true;
        for y in 0..HEIGHT {
            let cell = occupied(board, x, y);
            transitions += i32::from(cell != previous);
            previous = cell;
        }
        transitions += i32::from(!previous);
    }
    transitions
}

fn buried_holes(board: &Board, heights: &[i32; WIDTH]) -> i32 {
    let mut holes = 0;
    for (x, height) in heights.iter().copied().enumerate() {
        for y in 0..height {
            holes += i32::from(!occupied(
                board,
                x,
                usize::try_from(y).expect("height is nonnegative"),
            ));
        }
    }
    holes
}

fn cumulative_wells(board: &Board) -> i32 {
    let mut total = 0;
    for x in 0..WIDTH {
        let mut depth = 0;
        for y in 0..HEIGHT {
            let cell = occupied(board, x, y);
            let left_filled = x == 0 || occupied(board, x - 1, y);
            let right_filled = x + 1 == WIDTH || occupied(board, x + 1, y);
            if !cell && left_filled && right_filled {
                depth += 1;
                total += depth;
            } else {
                depth = 0;
            }
        }
    }
    total
}

fn occupied(board: &Board, x: usize, y: usize) -> bool {
    board
        .is_occupied(x, y)
        .expect("feature extraction visits only in-bounds cells")
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
}
