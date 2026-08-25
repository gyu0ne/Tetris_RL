use crate::FEATURE_COUNT;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActionToken {
    pub hold: bool,
    pub piece: String,
    pub orientation: String,
    pub x: i16,
    pub y: i16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObservationRecord {
    pub board_hex: String,
    pub garbage_hex: String,
    pub active: String,
    pub hold: Option<String>,
    pub hold_available: bool,
    pub preview: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImmediateEvents {
    pub lines_cleared: u8,
    pub perfect_clear: bool,
    pub cleared_garbage: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CandidateRecord {
    pub action: ActionToken,
    pub afterstate_checksum: u64,
    pub path_length: u16,
    pub features: [i32; FEATURE_COUNT],
    pub teacher_score: i64,
    pub rank: u16,
    pub immediate: ImmediateEvents,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TeacherRecord {
    pub id: String,
    pub config_hash: String,
    pub style: String,
    pub node_budget: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DecisionRecord {
    pub schema_version: String,
    pub rules_hash: String,
    pub engine_revision: String,
    pub mechanics_status: String,
    pub action_space: String,
    pub match_id: String,
    pub seed: u64,
    pub ply: u32,
    pub observation_hash: String,
    pub observation: ObservationRecord,
    pub teacher: TeacherRecord,
    pub candidates: Vec<CandidateRecord>,
    pub chosen_index: u16,
    pub top_two_margin: i64,
    pub terminal_outcome: Option<i8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DatasetManifest {
    pub schema_version: String,
    pub dataset_id: String,
    pub records_sha256: String,
    pub records_file: String,
    pub rules_id: String,
    pub rules_hash: String,
    pub engine_revision: String,
    pub mechanics_status: String,
    pub action_space: String,
    pub feature_names: Vec<String>,
    pub teacher: TeacherRecord,
    pub base_seed: u64,
    pub seed_stride: u64,
    pub requested_matches: u32,
    pub completed_matches: u32,
    pub requested_decisions_per_match: u32,
    pub decisions: u64,
    pub min_candidates: u16,
    pub max_candidates: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoloGenerationConfig {
    pub records_path: std::path::PathBuf,
    pub manifest_path: std::path::PathBuf,
    pub engine_revision: String,
    pub base_seed: u64,
    pub seed_stride: u64,
    pub matches: u32,
    pub decisions_per_match: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoloGenerationSummary {
    pub manifest: DatasetManifest,
}
