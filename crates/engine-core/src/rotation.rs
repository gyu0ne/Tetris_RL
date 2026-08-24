use crate::{Board, Orientation, PieceKind, PieceState};

/// Rotation inputs supported by the modern core.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RotationDirection {
    Clockwise,
    Counterclockwise,
    Half,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RotationResult {
    pub state: PieceState,
    pub kick_index: u8,
}

/// Ordered SRS+ candidate offsets using positive y upward.
///
/// The 90-degree JLSTZ values are Guideline SRS. I ordering and 180-degree
/// values follow publicly documented TETR.IO SRS+ observations and remain
/// `OBSERVED` until covered by current-version fixtures.
pub fn kick_tests(
    kind: PieceKind,
    from: Orientation,
    direction: RotationDirection,
) -> &'static [(i16, i16)] {
    if kind == PieceKind::O {
        return &[(0, 0)];
    }

    if direction == RotationDirection::Half {
        return half_kicks(kind, from);
    }

    if kind == PieceKind::I {
        return i_quarter_kicks(from, direction);
    }

    jlstz_quarter_kicks(from, direction)
}

pub fn try_rotate(
    board: &Board,
    state: PieceState,
    direction: RotationDirection,
) -> Option<RotationResult> {
    let orientation = match direction {
        RotationDirection::Clockwise => state.orientation.clockwise(),
        RotationDirection::Counterclockwise => state.orientation.counterclockwise(),
        RotationDirection::Half => state.orientation.half(),
    };
    let rotated = state.with_orientation(orientation);

    kick_tests(state.kind, state.orientation, direction)
        .iter()
        .copied()
        .enumerate()
        .find_map(|(index, (dx, dy))| {
            let candidate = rotated.translated(dx, dy);
            (!board.collides(candidate)).then_some(RotationResult {
                state: candidate,
                kick_index: index as u8,
            })
        })
}

fn jlstz_quarter_kicks(from: Orientation, direction: RotationDirection) -> &'static [(i16, i16)] {
    match (from, direction) {
        (Orientation::Spawn, RotationDirection::Clockwise) => {
            &[(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)]
        }
        (Orientation::Right, RotationDirection::Counterclockwise) => {
            &[(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)]
        }
        (Orientation::Right, RotationDirection::Clockwise) => {
            &[(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)]
        }
        (Orientation::Reverse, RotationDirection::Counterclockwise) => {
            &[(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)]
        }
        (Orientation::Reverse, RotationDirection::Clockwise) => {
            &[(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)]
        }
        (Orientation::Left, RotationDirection::Counterclockwise) => {
            &[(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)]
        }
        (Orientation::Left, RotationDirection::Clockwise) => {
            &[(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)]
        }
        (Orientation::Spawn, RotationDirection::Counterclockwise) => {
            &[(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)]
        }
        (_, RotationDirection::Half) => unreachable!("half rotations use a separate table"),
    }
}

fn i_quarter_kicks(from: Orientation, direction: RotationDirection) -> &'static [(i16, i16)] {
    match (from, direction) {
        (Orientation::Spawn, RotationDirection::Clockwise) => {
            &[(0, 0), (1, 0), (-2, 0), (-2, -1), (1, 2)]
        }
        (Orientation::Right, RotationDirection::Counterclockwise) => {
            &[(0, 0), (-1, 0), (2, 0), (-1, -2), (2, 1)]
        }
        (Orientation::Right, RotationDirection::Clockwise) => {
            &[(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)]
        }
        (Orientation::Reverse, RotationDirection::Counterclockwise) => {
            &[(0, 0), (-2, 0), (1, 0), (-2, 1), (1, -2)]
        }
        (Orientation::Reverse, RotationDirection::Clockwise) => {
            &[(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)]
        }
        (Orientation::Left, RotationDirection::Counterclockwise) => {
            &[(0, 0), (1, 0), (-2, 0), (1, 2), (-2, -1)]
        }
        (Orientation::Left, RotationDirection::Clockwise) => {
            &[(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)]
        }
        (Orientation::Spawn, RotationDirection::Counterclockwise) => {
            &[(0, 0), (-1, 0), (2, 0), (2, -1), (-1, 2)]
        }
        (_, RotationDirection::Half) => unreachable!("half rotations use a separate table"),
    }
}

fn half_kicks(kind: PieceKind, from: Orientation) -> &'static [(i16, i16)] {
    if kind == PieceKind::I {
        return match from {
            Orientation::Spawn => &[(0, 0), (0, 1)],
            Orientation::Right => &[(0, 0), (1, 0)],
            Orientation::Reverse => &[(0, 0), (0, -1)],
            Orientation::Left => &[(0, 0), (-1, 0)],
        };
    }

    match from {
        Orientation::Spawn => &[(0, 0), (0, 1), (1, 1), (-1, 1), (1, 0), (-1, 0)],
        Orientation::Right => &[(0, 0), (1, 0), (1, 2), (1, 1), (0, 2), (0, 1)],
        Orientation::Reverse => &[(0, 0), (0, -1), (-1, -1), (1, -1), (-1, 0), (1, 0)],
        Orientation::Left => &[(0, 0), (-1, 0), (-1, 2), (-1, 1), (0, 2), (0, 1)],
    }
}

#[cfg(test)]
mod tests {
    use super::{RotationDirection, kick_tests, try_rotate};
    use crate::{Board, Orientation, PieceKind, PieceState};

    #[test]
    fn open_space_rotation_uses_first_test() {
        let board = Board::empty();
        let piece = PieceState::new(PieceKind::T, Orientation::Spawn, 3, 10);
        let result =
            try_rotate(&board, piece, RotationDirection::Clockwise).expect("rotation fits");
        assert_eq!(result.state.orientation, Orientation::Right);
        assert_eq!(result.state.x, piece.x);
        assert_eq!(result.state.y, piece.y);
        assert_eq!(result.kick_index, 0);
    }

    #[test]
    fn wall_rotation_uses_ordered_kick() {
        let board = Board::empty();
        let piece = PieceState::new(PieceKind::T, Orientation::Right, -1, 4);
        assert!(!board.collides(piece));
        let result = try_rotate(&board, piece, RotationDirection::Counterclockwise)
            .expect("R to spawn should kick right");
        assert_eq!(result.state.x, 0);
        assert_eq!(result.kick_index, 1);
    }

    #[test]
    fn srs_plus_i_order_is_explicit() {
        assert_eq!(
            kick_tests(
                PieceKind::I,
                Orientation::Spawn,
                RotationDirection::Clockwise
            ),
            &[(0, 0), (1, 0), (-2, 0), (-2, -1), (1, 2)]
        );
    }

    #[test]
    fn half_rotation_has_six_non_i_tests() {
        assert_eq!(
            kick_tests(PieceKind::T, Orientation::Spawn, RotationDirection::Half).len(),
            6
        );
    }
}
