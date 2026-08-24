use crate::{Board, LastAction, Orientation, PieceKind, PieceState, RotationDirection};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpinMode {
    Disabled,
    AllMiniPlus,
}

/// Generic spin-classification parameters supplied by a versioned profile.
///
/// `t_full_kick_upgrade_mask` is deliberately explicit because the exact
/// target kick-index upgrade boundary still needs differential fixtures. Bit
/// `n` upgrades a three-corner T Mini produced by kick index `n` to Full.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpinRules {
    pub mode: SpinMode,
    pub t_full_kick_upgrade_mask: u16,
}

impl SpinRules {
    pub const fn disabled() -> Self {
        Self {
            mode: SpinMode::Disabled,
            t_full_kick_upgrade_mask: 0,
        }
    }

    pub const fn all_mini_plus_observed() -> Self {
        Self {
            mode: SpinMode::AllMiniPlus,
            t_full_kick_upgrade_mask: 0,
        }
    }
}

impl Default for SpinRules {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpinClassification {
    Mini,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpinOutcome {
    pub piece: PieceKind,
    pub classification: SpinClassification,
    pub direction: RotationDirection,
    pub kick_index: u8,
}

pub fn classify_spin(
    board: &Board,
    piece: PieceState,
    last_action: LastAction,
    rules: SpinRules,
) -> Option<SpinOutcome> {
    if rules.mode == SpinMode::Disabled {
        return None;
    }

    let LastAction::Rotation {
        direction,
        kick_index,
    } = last_action
    else {
        return None;
    };

    let classification = if piece.kind == PieceKind::T {
        classify_t_spin(board, piece, kick_index, rules)?
    } else if is_immobile(board, piece) {
        SpinClassification::Mini
    } else {
        return None;
    };

    Some(SpinOutcome {
        piece: piece.kind,
        classification,
        direction,
        kick_index,
    })
}

fn classify_t_spin(
    board: &Board,
    piece: PieceState,
    kick_index: u8,
    rules: SpinRules,
) -> Option<SpinClassification> {
    let center_x = piece.x + 1;
    let center_y = piece.y + 1;
    let corners = [
        (center_x - 1, center_y + 1),
        (center_x + 1, center_y + 1),
        (center_x - 1, center_y - 1),
        (center_x + 1, center_y - 1),
    ];
    let occupied = corners.map(|(x, y)| occupied_or_boundary(board, x, y));
    let occupied_count = occupied.into_iter().filter(|occupied| *occupied).count();

    if occupied_count >= 3 {
        let front = match piece.orientation {
            Orientation::Spawn => [occupied[0], occupied[1]],
            Orientation::Right => [occupied[1], occupied[3]],
            Orientation::Reverse => [occupied[2], occupied[3]],
            Orientation::Left => [occupied[0], occupied[2]],
        };
        let upgrade_bit = 1_u16.checked_shl(u32::from(kick_index)).unwrap_or(0);
        if front.into_iter().all(|corner| corner)
            || rules.t_full_kick_upgrade_mask & upgrade_bit != 0
        {
            Some(SpinClassification::Full)
        } else {
            Some(SpinClassification::Mini)
        }
    } else if is_immobile(board, piece) {
        // BETA 1.5.0 All-Mini+ path: an immobile T rotation that does not
        // satisfy the three-corner test is still a Mini.
        Some(SpinClassification::Mini)
    } else {
        None
    }
}

fn is_immobile(board: &Board, piece: PieceState) -> bool {
    [(-1, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .all(|(dx, dy)| board.collides(piece.translated(dx, dy)))
}

fn occupied_or_boundary(board: &Board, x: i16, y: i16) -> bool {
    let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
        return true;
    };
    board.is_occupied(x, y).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::{SpinClassification, SpinRules, classify_spin};
    use crate::{Board, HEIGHT, LastAction, Orientation, PieceKind, PieceState, RotationDirection};

    fn rotation() -> LastAction {
        LastAction::Rotation {
            direction: RotationDirection::Clockwise,
            kick_index: 0,
        }
    }

    #[test]
    fn t_front_corners_classify_full() {
        let mut rows = [0; HEIGHT];
        rows[1] = (1 << 3) | (1 << 5);
        let board = Board::from_rows(rows).expect("valid board");
        let piece = PieceState::new(PieceKind::T, Orientation::Spawn, 3, -1);

        let spin = classify_spin(
            &board,
            piece,
            rotation(),
            SpinRules::all_mini_plus_observed(),
        )
        .expect("three-corner T spin");
        assert_eq!(spin.classification, SpinClassification::Full);
    }

    #[test]
    fn t_three_corner_without_both_front_corners_is_mini() {
        let mut rows = [0; HEIGHT];
        rows[1] = 1 << 3;
        let board = Board::from_rows(rows).expect("valid board");
        let piece = PieceState::new(PieceKind::T, Orientation::Spawn, 3, -1);

        let spin = classify_spin(
            &board,
            piece,
            rotation(),
            SpinRules::all_mini_plus_observed(),
        )
        .expect("three-corner T mini");
        assert_eq!(spin.classification, SpinClassification::Mini);
    }

    #[test]
    fn explicit_kick_mask_can_upgrade_a_three_corner_t_mini() {
        let mut rows = [0; HEIGHT];
        rows[1] = 1 << 3;
        let board = Board::from_rows(rows).expect("valid board");
        let piece = PieceState::new(PieceKind::T, Orientation::Spawn, 3, -1);
        let rules = SpinRules {
            mode: super::SpinMode::AllMiniPlus,
            t_full_kick_upgrade_mask: 1 << 4,
        };
        let action = LastAction::Rotation {
            direction: RotationDirection::Clockwise,
            kick_index: 4,
        };

        let spin = classify_spin(&board, piece, action, rules).expect("upgraded T spin");
        assert_eq!(spin.classification, SpinClassification::Full);
    }

    #[test]
    fn immobile_non_t_rotation_is_all_mini_plus_mini() {
        let mut rows = [0; HEIGHT];
        rows[0] = (1 << 3) | (1 << 6);
        rows[2] = 1 << 4;
        let board = Board::from_rows(rows).expect("valid board");
        let piece = PieceState::new(PieceKind::O, Orientation::Spawn, 3, -1);

        let spin = classify_spin(
            &board,
            piece,
            rotation(),
            SpinRules::all_mini_plus_observed(),
        )
        .expect("immobile O mini");
        assert_eq!(spin.classification, SpinClassification::Mini);
    }

    #[test]
    fn translation_cannot_be_a_spin() {
        let board = Board::empty();
        let piece = PieceState::new(PieceKind::T, Orientation::Spawn, 3, -1);
        assert_eq!(
            classify_spin(
                &board,
                piece,
                LastAction::Translation,
                SpinRules::all_mini_plus_observed(),
            ),
            None
        );
    }
}
