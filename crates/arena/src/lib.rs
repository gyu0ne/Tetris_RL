//! Placement-level bot arena primitives.
//!
//! The policy action is a locked afterstate token, not a sequence of frame
//! inputs. Geometry paths remain diagnostics only and are never model inputs.

#![forbid(unsafe_code)]

mod closed_loop;
mod features;
mod record;
mod solo;
mod teacher;

pub use closed_loop::{CandidateBatch, ClosedLoopError, SoloBatch};
pub use features::{FEATURE_COUNT, FEATURE_NAMES, extract_afterstate_features};
pub use record::{
    ActionToken, CandidateRecord, DatasetManifest, DecisionRecord, ImmediateEvents,
    ObservationRecord, SoloGenerationConfig, SoloGenerationSummary, TeacherRecord,
};
pub use solo::{GenerationError, generate_solo_dataset};
pub use teacher::{DELLACHERIE_SCALED_WEIGHTS, LinearTeacher, TeacherError};

pub const DATASET_SCHEMA_VERSION: &str = "solo-afterstate-imitation-v1";
pub const ACTION_SPACE_ID: &str = "geometric-locked-afterstate-v1";
pub const MECHANICS_STATUS: &str = "OBSERVED_NOT_FUNCTIONALLY_VERIFIED";
pub const RULES_ID: &str = "tetrio-beta-1.7.8-solo-bootstrap-observed-v1";

/// Canonical text hashed into every v1 manifest. Any transition-relevant
/// default change must create a new rules ID and canonical text.
pub const RULES_CANONICAL: &str = concat!(
    "rules_id=tetrio-beta-1.7.8-solo-bootstrap-observed-v1\n",
    "game_config=engine_core_default\n",
    "action_space=geometric_locked_afterstate_v1\n",
    "timing=placement_level_no_frame_budget\n",
    "scoring=none\n",
);
