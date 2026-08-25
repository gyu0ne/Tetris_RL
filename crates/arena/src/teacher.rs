use crate::FEATURE_COUNT;
use std::fmt;

/// Dellacherie six-feature coefficients scaled to integer milli-units. The
/// landing-height coefficient is halved because v1 stores twice the height.
pub const DELLACHERIE_SCALED_WEIGHTS: [i64; FEATURE_COUNT] =
    [-2_250, 3_418, -3_218, -9_349, -7_899, -3_386, 0, 0, 0, 0];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinearTeacher {
    weights: [i64; FEATURE_COUNT],
}

impl LinearTeacher {
    pub const fn dellacherie_v1() -> Self {
        Self {
            weights: DELLACHERIE_SCALED_WEIGHTS,
        }
    }

    pub const fn weights(self) -> [i64; FEATURE_COUNT] {
        self.weights
    }

    pub fn score(self, features: [i32; FEATURE_COUNT]) -> Result<i64, TeacherError> {
        self.weights
            .into_iter()
            .zip(features)
            .try_fold(0_i64, |sum, (weight, value)| {
                let term = weight
                    .checked_mul(i64::from(value))
                    .ok_or(TeacherError::Overflow)?;
                sum.checked_add(term).ok_or(TeacherError::Overflow)
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeacherError {
    Overflow,
}

impl fmt::Display for TeacherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => formatter.write_str("linear teacher score overflowed i64"),
        }
    }
}

impl std::error::Error for TeacherError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dellacherie_teacher_is_deterministic_and_prefers_fewer_holes() {
        let teacher = LinearTeacher::dellacherie_v1();
        let clean = [2, 0, 4, 4, 0, 0, 4, 0, 1, 0];
        let holes = [2, 0, 4, 4, 2, 0, 4, 0, 1, 0];

        assert_eq!(teacher.score(clean).unwrap(), teacher.score(clean).unwrap());
        assert!(teacher.score(clean).unwrap() > teacher.score(holes).unwrap());
    }
}
