use crate::{
    ACTION_SPACE_ID, ActionToken, CandidateRecord, DATASET_SCHEMA_VERSION, DatasetManifest,
    DecisionRecord, FEATURE_NAMES, ImmediateEvents, LinearTeacher, MECHANICS_STATUS,
    ObservationRecord, RULES_CANONICAL, RULES_ID, SoloGenerationConfig, SoloGenerationSummary,
    TeacherError, TeacherRecord, extract_afterstate_features,
};
use engine_core::{GameConfig, GameError, GameState, Orientation, PieceKind, PieceState};
use flate2::{Compression, GzBuilder};
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

const TEACHER_ID: &str = "dellacherie-linear-v1";
const TEACHER_STYLE: &str = "solo-stack-survival";

pub fn generate_solo_dataset(
    config: &SoloGenerationConfig,
) -> Result<SoloGenerationSummary, GenerationError> {
    validate_config(config)?;
    create_parent(&config.records_path)?;
    create_parent(&config.manifest_path)?;

    let rules_hash = sha256_hex(RULES_CANONICAL.as_bytes());
    let teacher = LinearTeacher::dellacherie_v1();
    let teacher_record = teacher_record(teacher);
    let records_file = File::create(&config.records_path)?;
    let mut writer = GzBuilder::new()
        .mtime(0)
        .write(BufWriter::new(records_file), Compression::new(6));
    let game_config = GameConfig::default();
    let mut decisions = 0_u64;
    let mut min_candidates = u16::MAX;
    let mut max_candidates = 0_u16;

    for match_index in 0..config.matches {
        let seed = u64::from(match_index)
            .checked_mul(config.seed_stride)
            .and_then(|offset| config.base_seed.checked_add(offset))
            .ok_or(GenerationError::InvalidConfig(
                "seed schedule must not overflow u64",
            ))?;
        let match_id = format!("solo-{seed:016x}");
        let mut game = GameState::new(seed, game_config)?;

        for ply in 0..config.decisions_per_match {
            if game.is_top_out() {
                break;
            }
            let observation = observation_record(&game);
            let observation_hash = hash_serializable(&observation)?;
            let mut choices = enumerate_candidates(&game, game_config, teacher)?;
            if choices.is_empty() {
                break;
            }
            let (chosen_index, margin) = rank_candidates(&mut choices)?;
            let candidate_count = u16::try_from(choices.len())
                .map_err(|_| GenerationError::CandidateCountOverflow(choices.len()))?;
            min_candidates = min_candidates.min(candidate_count);
            max_candidates = max_candidates.max(candidate_count);

            let record = DecisionRecord {
                schema_version: DATASET_SCHEMA_VERSION.to_owned(),
                rules_hash: rules_hash.clone(),
                engine_revision: config.engine_revision.clone(),
                mechanics_status: MECHANICS_STATUS.to_owned(),
                action_space: ACTION_SPACE_ID.to_owned(),
                match_id: match_id.clone(),
                seed,
                ply,
                observation_hash,
                observation,
                teacher: teacher_record.clone(),
                candidates: choices.iter().map(|choice| choice.record.clone()).collect(),
                chosen_index,
                top_two_margin: margin,
                terminal_outcome: None,
            };
            serde_json::to_writer(&mut writer, &record)?;
            writer.write_all(b"\n")?;
            decisions += 1;

            let chosen = &choices[usize::from(chosen_index)];
            if chosen.record.action.hold {
                game.hold_active()?;
            }
            game.lock_placement(chosen.placement)?;
        }
    }
    let mut records_file = writer.finish()?;
    records_file.flush()?;
    drop(records_file);

    let records_sha256 = sha256_file(&config.records_path)?;
    let records_file = config
        .records_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| GenerationError::InvalidRecordsFile(config.records_path.clone()))?
        .to_owned();
    let manifest = DatasetManifest {
        schema_version: DATASET_SCHEMA_VERSION.to_owned(),
        dataset_id: records_sha256.clone(),
        records_sha256,
        records_file,
        rules_id: RULES_ID.to_owned(),
        rules_hash,
        engine_revision: config.engine_revision.clone(),
        mechanics_status: MECHANICS_STATUS.to_owned(),
        action_space: ACTION_SPACE_ID.to_owned(),
        feature_names: FEATURE_NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        teacher: teacher_record,
        base_seed: config.base_seed,
        seed_stride: config.seed_stride,
        requested_matches: config.matches,
        completed_matches: config.matches,
        requested_decisions_per_match: config.decisions_per_match,
        decisions,
        min_candidates: if decisions == 0 { 0 } else { min_candidates },
        max_candidates,
    };
    let mut manifest_writer = BufWriter::new(File::create(&config.manifest_path)?);
    serde_json::to_writer_pretty(&mut manifest_writer, &manifest)?;
    manifest_writer.write_all(b"\n")?;
    manifest_writer.flush()?;

    Ok(SoloGenerationSummary { manifest })
}

#[derive(Clone)]
pub(crate) struct CandidateChoice {
    pub(crate) record: CandidateRecord,
    pub(crate) placement: PieceState,
}

pub(crate) fn enumerate_candidates(
    game: &GameState,
    game_config: GameConfig,
    teacher: LinearTeacher,
) -> Result<Vec<CandidateChoice>, GenerationError> {
    let mut sources = vec![(false, game.clone())];
    if game.hold_available() {
        let mut held = game.clone();
        let outcome = held.hold_active()?;
        if !outcome.top_out {
            sources.push((true, held));
        }
    }

    let mut choices = Vec::new();
    for (used_hold, source) in sources {
        let next_piece = source.preview().first().copied();
        for placement in source.reachable_placements() {
            let mut board = *source.board();
            let lock = board.lock(placement.state)?;
            let features = extract_afterstate_features(&board, placement.state, lock.cleared);
            let mut score = teacher.score(features)?;
            if next_piece.is_some_and(|kind| board.collides(game_config.spawn.piece(kind)))
                && !(game_config.clutch_clear && lock.cleared.count() > 0)
            {
                score = score.saturating_sub(1_000_000_000);
            }
            let action = action_token(used_hold, placement.state);
            let path_length = u16::try_from(placement.path.len())
                .map_err(|_| GenerationError::PathLengthOverflow(placement.path.len()))?;
            choices.push(CandidateChoice {
                record: CandidateRecord {
                    action,
                    afterstate_checksum: board.checksum(),
                    path_length,
                    features,
                    teacher_score: score,
                    rank: 0,
                    immediate: ImmediateEvents {
                        lines_cleared: lock.cleared.count(),
                        perfect_clear: lock.perfect_clear,
                        cleared_garbage: lock.cleared_garbage,
                    },
                },
                placement: placement.state,
            });
        }
    }
    Ok(choices)
}

fn rank_candidates(choices: &mut [CandidateChoice]) -> Result<(u16, i64), GenerationError> {
    let mut order: Vec<usize> = (0..choices.len()).collect();
    order.sort_by_key(|index| {
        let record = &choices[*index].record;
        (
            Reverse(record.teacher_score),
            record.path_length,
            action_sort_key(&record.action),
        )
    });
    for (rank, index) in order.iter().copied().enumerate() {
        choices[index].record.rank =
            u16::try_from(rank).map_err(|_| GenerationError::CandidateCountOverflow(rank))?;
    }
    let chosen = *order.first().ok_or(GenerationError::NoCandidates)?;
    let chosen_index =
        u16::try_from(chosen).map_err(|_| GenerationError::CandidateCountOverflow(chosen))?;
    let margin = order.get(1).map_or(0, |runner_up| {
        choices[chosen].record.teacher_score - choices[*runner_up].record.teacher_score
    });
    Ok((chosen_index, margin))
}

fn observation_record(game: &GameState) -> ObservationRecord {
    ObservationRecord {
        board_hex: board_hex(game.board().rows()),
        garbage_hex: board_hex(game.board().garbage_rows()),
        active: piece_name(game.active().kind).to_owned(),
        hold: game.hold().map(|kind| piece_name(kind).to_owned()),
        hold_available: game.hold_available(),
        preview: game
            .preview()
            .iter()
            .map(|kind| piece_name(*kind).to_owned())
            .collect(),
    }
}

fn action_token(hold: bool, placement: PieceState) -> ActionToken {
    ActionToken {
        hold,
        piece: piece_name(placement.kind).to_owned(),
        orientation: orientation_name(placement.orientation).to_owned(),
        x: placement.x,
        y: placement.y,
    }
}

fn action_sort_key(action: &ActionToken) -> (bool, &str, &str, i16, i16) {
    (
        action.hold,
        action.piece.as_str(),
        action.orientation.as_str(),
        action.x,
        action.y,
    )
}

fn piece_name(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::I => "I",
        PieceKind::J => "J",
        PieceKind::L => "L",
        PieceKind::O => "O",
        PieceKind::S => "S",
        PieceKind::T => "T",
        PieceKind::Z => "Z",
    }
}

fn orientation_name(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Spawn => "spawn",
        Orientation::Right => "right",
        Orientation::Reverse => "reverse",
        Orientation::Left => "left",
    }
}

fn board_hex(rows: &[u16]) -> String {
    let mut value = String::with_capacity(rows.len() * 4);
    for row in rows {
        use std::fmt::Write as _;
        write!(&mut value, "{row:04x}").expect("writing to String cannot fail");
    }
    value
}

fn hash_serializable<T: serde::Serialize>(value: &T) -> Result<String, GenerationError> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
}

fn teacher_record(teacher: LinearTeacher) -> TeacherRecord {
    let mut canonical = String::from("teacher=dellacherie-linear-v1\nscale=milli\n");
    for (name, weight) in FEATURE_NAMES.iter().zip(teacher.weights()) {
        use std::fmt::Write as _;
        writeln!(&mut canonical, "{name}={weight}").expect("writing to String cannot fail");
    }
    TeacherRecord {
        id: TEACHER_ID.to_owned(),
        config_hash: sha256_hex(canonical.as_bytes()),
        style: TEACHER_STYLE.to_owned(),
        node_budget: 0,
    }
}

fn validate_config(config: &SoloGenerationConfig) -> Result<(), GenerationError> {
    if config.matches == 0 {
        return Err(GenerationError::InvalidConfig("matches must be positive"));
    }
    if config.decisions_per_match == 0 {
        return Err(GenerationError::InvalidConfig(
            "decisions_per_match must be positive",
        ));
    }
    if config.seed_stride == 0 {
        return Err(GenerationError::InvalidConfig(
            "seed_stride must be positive",
        ));
    }
    if u64::from(config.matches - 1)
        .checked_mul(config.seed_stride)
        .and_then(|offset| config.base_seed.checked_add(offset))
        .is_none()
    {
        return Err(GenerationError::InvalidConfig(
            "seed schedule must not overflow u64",
        ));
    }
    if config.engine_revision.trim().is_empty() {
        return Err(GenerationError::InvalidConfig(
            "engine_revision must not be empty",
        ));
    }
    if config
        .records_path
        .extension()
        .and_then(|value| value.to_str())
        != Some("gz")
    {
        return Err(GenerationError::InvalidConfig(
            "records_path must use the deterministic .jsonl.gz format",
        ));
    }
    Ok(())
}

fn create_parent(path: &Path) -> Result<(), GenerationError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, GenerationError> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex_digest(digest.finalize().as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_digest(digest.as_slice())
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

#[derive(Debug)]
pub enum GenerationError {
    Io(io::Error),
    Json(serde_json::Error),
    Game(GameError),
    Teacher(TeacherError),
    InvalidConfig(&'static str),
    InvalidRecordsFile(std::path::PathBuf),
    NoCandidates,
    CandidateCountOverflow(usize),
    PathLengthOverflow(usize),
}

impl fmt::Display for GenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "dataset I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "dataset JSON failed: {error}"),
            Self::Game(error) => write!(formatter, "engine transition failed: {error}"),
            Self::Teacher(error) => write!(formatter, "teacher evaluation failed: {error}"),
            Self::InvalidConfig(message) => {
                write!(formatter, "invalid generation config: {message}")
            }
            Self::InvalidRecordsFile(path) => {
                write!(
                    formatter,
                    "records path has no UTF-8 file name: {}",
                    path.display()
                )
            }
            Self::NoCandidates => formatter.write_str("playable state produced no candidates"),
            Self::CandidateCountOverflow(count) => {
                write!(formatter, "candidate count does not fit u16: {count}")
            }
            Self::PathLengthOverflow(length) => {
                write!(
                    formatter,
                    "diagnostic path length does not fit u16: {length}"
                )
            }
        }
    }
}

impl std::error::Error for GenerationError {}

impl From<io::Error> for GenerationError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for GenerationError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<GameError> for GenerationError {
    fn from(value: GameError) -> Self {
        Self::Game(value)
    }
}

impl From<engine_core::BoardError> for GenerationError {
    fn from(value: engine_core::BoardError) -> Self {
        Self::Game(GameError::Board(value))
    }
}

impl From<TeacherError> for GenerationError {
    fn from(value: TeacherError) -> Self {
        Self::Teacher(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_byte_deterministic_and_records_all_ranked_candidates() {
        let root = std::env::temp_dir().join(format!("tetris-arena-{}", std::process::id()));
        let first = config(
            root.join("first.jsonl.gz"),
            root.join("first.manifest.json"),
        );
        let second = config(
            root.join("second.jsonl.gz"),
            root.join("second.manifest.json"),
        );

        let one = generate_solo_dataset(&first).unwrap();
        let two = generate_solo_dataset(&second).unwrap();
        let first_bytes = fs::read(&first.records_path).unwrap();
        let second_bytes = fs::read(&second.records_path).unwrap();
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(one.manifest.records_sha256, two.manifest.records_sha256);
        assert_eq!(one.manifest.seed_stride, 104_729);

        let decoder = flate2::read::GzDecoder::new(first_bytes.as_slice());
        let record: DecisionRecord = serde_json::Deserializer::from_reader(decoder)
            .into_iter()
            .next()
            .unwrap()
            .unwrap();
        assert!(record.candidates.len() > 1);
        assert_eq!(record.candidates[usize::from(record.chosen_index)].rank, 0);
        assert!(
            record
                .candidates
                .iter()
                .all(|candidate| candidate.path_length > 0)
        );
        assert_eq!(record.action_space, ACTION_SPACE_ID);
        assert_eq!(record.mechanics_status, MECHANICS_STATUS);

        for path in [
            first.records_path,
            first.manifest_path,
            second.records_path,
            second.manifest_path,
        ] {
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }

    fn config(
        records_path: std::path::PathBuf,
        manifest_path: std::path::PathBuf,
    ) -> SoloGenerationConfig {
        SoloGenerationConfig {
            records_path,
            manifest_path,
            engine_revision: "test-revision".to_owned(),
            base_seed: 7,
            seed_stride: 104_729,
            matches: 2,
            decisions_per_match: 3,
        }
    }
}
