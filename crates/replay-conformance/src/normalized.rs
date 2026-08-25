use crate::{
    BattleEventsSnapshot, BattlePlayerEventsSnapshot, BattlePlayerSnapshot, BattleSnapshot,
    FrameSnapshot, FunctionalCaseKind, FunctionalConformanceCase, MechanicClaim, ReferenceEvidence,
    ReferenceTrace, TimingSnapshot,
};
use engine_core::{
    HEIGHT, LastAction, Orientation, PieceKind, PieceState, RotationDirection, TopOutReason, WIDTH,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fmt};
use versus::{
    AttackMultiplier, AttackOutcome, AttackPacket, AttackPacketKind, AttackPackets, AttackState,
    BattleResult, GarbageCancellationOutcome, GarbageInsertionOutcome, IncomingGarbagePacket,
};

pub const NORMALIZED_TRACE_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalizedDocument {
    Manifest,
    Trace,
}

impl fmt::Display for NormalizedDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Manifest => "manifest",
            Self::Trace => "trace",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NormalizedFixtureError {
    Json {
        document: NormalizedDocument,
        message: String,
    },
    UnsupportedSchema {
        document: NormalizedDocument,
        actual: u16,
    },
    InvalidSha256 {
        field: &'static str,
    },
    TraceHashMismatch {
        expected: String,
        actual: String,
    },
    InvalidField {
        field: String,
        reason: &'static str,
    },
    TraceKindMismatch {
        reason: &'static str,
    },
    ActualTraceKindMismatch {
        expected: &'static str,
        actual: &'static str,
    },
}

impl fmt::Display for NormalizedFixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json { document, message } => {
                write!(formatter, "invalid {document} JSON: {message}")
            }
            Self::UnsupportedSchema { document, actual } => write!(
                formatter,
                "unsupported {document} schema {actual}; expected {NORMALIZED_TRACE_SCHEMA_VERSION}"
            ),
            Self::InvalidSha256 { field } => {
                write!(
                    formatter,
                    "{field} must be exactly 64 hexadecimal characters"
                )
            }
            Self::TraceHashMismatch { expected, actual } => write!(
                formatter,
                "trace_sha256 mismatch: expected {expected}, calculated {actual}"
            ),
            Self::InvalidField { field, reason } => {
                write!(formatter, "invalid field {field}: {reason}")
            }
            Self::TraceKindMismatch { reason } => formatter.write_str(reason),
            Self::ActualTraceKindMismatch { expected, actual } => {
                write!(formatter, "fixture contains {expected} trace, not {actual}")
            }
        }
    }
}

impl std::error::Error for NormalizedFixtureError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadedReferenceTrace {
    Solo(Vec<FrameSnapshot>),
    Battle(Vec<BattleSnapshot>),
}

impl LoadedReferenceTrace {
    pub fn len(&self) -> usize {
        match self {
            Self::Solo(trace) => trace.len(),
            Self::Battle(trace) => trace.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Solo(_) => "solo",
            Self::Battle(_) => "battle",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedReferenceFixture {
    pub id: String,
    pub kind: FunctionalCaseKind,
    pub evidence: ReferenceEvidence,
    pub trace_sha256: String,
    pub claims: Vec<MechanicClaim>,
    pub trace: LoadedReferenceTrace,
}

impl LoadedReferenceFixture {
    pub fn bind_solo<'a>(
        &'a self,
        actual: &'a [FrameSnapshot],
    ) -> Result<FunctionalConformanceCase<'a>, NormalizedFixtureError> {
        let LoadedReferenceTrace::Solo(expected) = &self.trace else {
            return Err(NormalizedFixtureError::ActualTraceKindMismatch {
                expected: "battle",
                actual: "solo",
            });
        };
        Ok(self.case(ReferenceTrace::Solo { expected, actual }))
    }

    pub fn bind_battle<'a>(
        &'a self,
        actual: &'a [BattleSnapshot],
    ) -> Result<FunctionalConformanceCase<'a>, NormalizedFixtureError> {
        let LoadedReferenceTrace::Battle(expected) = &self.trace else {
            return Err(NormalizedFixtureError::ActualTraceKindMismatch {
                expected: "solo",
                actual: "battle",
            });
        };
        Ok(self.case(ReferenceTrace::Battle { expected, actual }))
    }

    fn case<'a>(&'a self, trace: ReferenceTrace<'a>) -> FunctionalConformanceCase<'a> {
        FunctionalConformanceCase {
            id: self.id.clone(),
            kind: self.kind,
            evidence: self.evidence.clone(),
            claims: self.claims.clone(),
            trace,
        }
    }
}

/// Loads an adapter-produced trace only after verifying both its schema and
/// the SHA-256 of the exact trace bytes. The source artifact hash identifies
/// the replay/capture used by the external adapter; it is not substituted with
/// the normalized trace hash.
pub fn load_normalized_fixture(
    manifest_bytes: &[u8],
    trace_bytes: &[u8],
) -> Result<LoadedReferenceFixture, NormalizedFixtureError> {
    let manifest: WireManifest =
        serde_json::from_slice(manifest_bytes).map_err(|error| NormalizedFixtureError::Json {
            document: NormalizedDocument::Manifest,
            message: error.to_string(),
        })?;
    if manifest.schema_version != NORMALIZED_TRACE_SCHEMA_VERSION {
        return Err(NormalizedFixtureError::UnsupportedSchema {
            document: NormalizedDocument::Manifest,
            actual: manifest.schema_version,
        });
    }
    require_sha256(&manifest.source_artifact_sha256, "source_artifact_sha256")?;
    require_sha256(&manifest.trace_sha256, "trace_sha256")?;

    let actual_trace_hash = hex_sha256(trace_bytes);
    if !manifest
        .trace_sha256
        .eq_ignore_ascii_case(&actual_trace_hash)
    {
        return Err(NormalizedFixtureError::TraceHashMismatch {
            expected: manifest.trace_sha256,
            actual: actual_trace_hash,
        });
    }

    validate_nonempty(&manifest.case_id, "case_id")?;
    validate_nonempty(&manifest.target_profile, "target_profile")?;
    validate_nonempty(&manifest.reference_build, "reference_build")?;
    validate_nonempty(&manifest.source, "source")?;

    let kind = parse_case_kind(&manifest.case_kind)?;
    let claims = parse_claims(&manifest.claims)?;
    let wire_trace: WireTrace =
        serde_json::from_slice(trace_bytes).map_err(|error| NormalizedFixtureError::Json {
            document: NormalizedDocument::Trace,
            message: error.to_string(),
        })?;
    let trace = convert_trace(wire_trace)?;

    if kind == FunctionalCaseKind::RandomizedBattle
        && !matches!(trace, LoadedReferenceTrace::Battle(_))
    {
        return Err(NormalizedFixtureError::TraceKindMismatch {
            reason: "randomized_battle manifest requires a battle trace",
        });
    }
    if claims.iter().any(|claim| claim.requires_battle_trace())
        && !matches!(trace, LoadedReferenceTrace::Battle(_))
    {
        return Err(NormalizedFixtureError::TraceKindMismatch {
            reason: "a battle-only mechanics claim requires a battle trace",
        });
    }

    Ok(LoadedReferenceFixture {
        id: manifest.case_id,
        kind,
        evidence: ReferenceEvidence {
            target_profile: manifest.target_profile,
            reference_build: manifest.reference_build,
            source: manifest.source,
            artifact_sha256: manifest.source_artifact_sha256.to_ascii_lowercase(),
        },
        trace_sha256: actual_trace_hash,
        claims,
        trace,
    })
}

fn convert_trace(wire: WireTrace) -> Result<LoadedReferenceTrace, NormalizedFixtureError> {
    match wire {
        WireTrace::Solo {
            schema_version,
            snapshots,
        } => {
            require_trace_schema(schema_version)?;
            let converted = snapshots
                .into_iter()
                .enumerate()
                .map(|(index, snapshot)| convert_frame(snapshot, &format!("snapshots[{index}]")))
                .collect::<Result<Vec<_>, _>>()?;
            validate_frames(&converted)?;
            Ok(LoadedReferenceTrace::Solo(converted))
        }
        WireTrace::Battle {
            schema_version,
            snapshots,
        } => {
            require_trace_schema(schema_version)?;
            let converted = snapshots
                .into_iter()
                .enumerate()
                .map(|(index, snapshot)| convert_battle(snapshot, index))
                .collect::<Result<Vec<_>, _>>()?;
            validate_battle_frames(&converted)?;
            Ok(LoadedReferenceTrace::Battle(converted))
        }
    }
}

fn require_trace_schema(actual: u16) -> Result<(), NormalizedFixtureError> {
    if actual != NORMALIZED_TRACE_SCHEMA_VERSION {
        return Err(NormalizedFixtureError::UnsupportedSchema {
            document: NormalizedDocument::Trace,
            actual,
        });
    }
    Ok(())
}

fn convert_frame(
    wire: WireFrameSnapshot,
    path: &str,
) -> Result<FrameSnapshot, NormalizedFixtureError> {
    let board_rows = convert_rows(wire.board_rows, &format!("{path}.board_rows"))?;
    let garbage_rows = convert_rows(wire.garbage_rows, &format!("{path}.garbage_rows"))?;
    for row in 0..HEIGHT {
        if garbage_rows[row] & !board_rows[row] != 0 {
            return invalid(
                format!("{path}.garbage_rows[{row}]"),
                "garbage cells must be a subset of occupied cells",
            );
        }
    }
    let timing = wire
        .timing
        .map(|timing| convert_timing(timing, &format!("{path}.timing")))
        .transpose()?;
    Ok(FrameSnapshot {
        frame: wire.frame,
        board_rows,
        garbage_rows,
        active: convert_piece(wire.active, &format!("{path}.active"))?,
        hold: wire
            .hold
            .map(|value| parse_piece_kind(&value, &format!("{path}.hold")))
            .transpose()?,
        preview: wire
            .preview
            .iter()
            .enumerate()
            .map(|(index, value)| parse_piece_kind(value, &format!("{path}.preview[{index}]")))
            .collect::<Result<Vec<_>, _>>()?,
        top_out: wire
            .top_out
            .map(|value| parse_top_out(&value, &format!("{path}.top_out")))
            .transpose()?,
        timing,
    })
}

fn convert_battle(
    wire: WireBattleSnapshot,
    index: usize,
) -> Result<BattleSnapshot, NormalizedFixtureError> {
    let path = format!("snapshots[{index}]");
    let player_one = convert_battle_player(wire.player_one, &format!("{path}.player_one"))?;
    let player_two = convert_battle_player(wire.player_two, &format!("{path}.player_two"))?;
    if player_one.game.frame != wire.frame || player_two.game.frame != wire.frame {
        return invalid(
            format!("{path}.frame"),
            "outer and both player game frame numbers must match",
        );
    }
    let result = parse_battle_result(&wire.result, &format!("{path}.result"))?;
    let events = wire
        .events
        .map(|events| convert_events(events, &format!("{path}.events")))
        .transpose()?;
    if let Some(events) = events
        && (events.frame != wire.frame || events.result != result)
    {
        return invalid(
            format!("{path}.events"),
            "event frame and result must match the enclosing snapshot",
        );
    }
    let multiplier_bits = parse_bits(
        &wire.garbage_multiplier_bits,
        &format!("{path}.garbage_multiplier_bits"),
    )?;
    let garbage_multiplier = AttackMultiplier::from_ieee_bits(multiplier_bits).map_err(|_| {
        NormalizedFixtureError::InvalidField {
            field: format!("{path}.garbage_multiplier_bits"),
            reason: "multiplier must be finite",
        }
    })?;
    if garbage_multiplier.value().is_sign_negative() {
        return invalid(
            format!("{path}.garbage_multiplier_bits"),
            "multiplier must be non-negative",
        );
    }
    Ok(BattleSnapshot {
        frame: wire.frame,
        player_one,
        player_two,
        garbage_multiplier,
        result,
        events,
    })
}

fn convert_battle_player(
    wire: WireBattlePlayerSnapshot,
    path: &str,
) -> Result<BattlePlayerSnapshot, NormalizedFixtureError> {
    Ok(BattlePlayerSnapshot {
        game: convert_frame(wire.game, &format!("{path}.game"))?,
        attack: AttackState {
            combo: wire.attack.combo,
            back_to_back: wire.attack.back_to_back,
        },
        incoming: wire
            .incoming
            .into_iter()
            .enumerate()
            .map(|(index, packet)| convert_incoming(packet, &format!("{path}.incoming[{index}]")))
            .collect::<Result<Vec<_>, _>>()?,
        sent_lines: wire.sent_lines,
    })
}

fn convert_events(
    wire: WireBattleEventsSnapshot,
    path: &str,
) -> Result<BattleEventsSnapshot, NormalizedFixtureError> {
    Ok(BattleEventsSnapshot {
        frame: wire.frame,
        player_one: convert_player_events(wire.player_one, &format!("{path}.player_one"))?,
        player_two: convert_player_events(wire.player_two, &format!("{path}.player_two"))?,
        result: parse_battle_result(&wire.result, &format!("{path}.result"))?,
    })
}

fn convert_player_events(
    wire: WireBattlePlayerEventsSnapshot,
    path: &str,
) -> Result<BattlePlayerEventsSnapshot, NormalizedFixtureError> {
    Ok(BattlePlayerEventsSnapshot {
        attack: wire
            .attack
            .map(|attack| convert_attack_outcome(attack, &format!("{path}.attack")))
            .transpose()?,
        cancellation: wire
            .cancellation
            .map(|cancellation| convert_cancellation(cancellation, &format!("{path}.cancellation")))
            .transpose()?,
        insertion: wire.insertion.map(|insertion| GarbageInsertionOutcome {
            inserted: insertion.inserted,
            overflowed_buffer: insertion.overflowed_buffer,
            blocked_by_clear: insertion.blocked_by_clear,
        }),
        transmitted: convert_packets(wire.transmitted, &format!("{path}.transmitted"))?,
    })
}

fn convert_attack_outcome(
    wire: WireAttackOutcome,
    path: &str,
) -> Result<AttackOutcome, NormalizedFixtureError> {
    Ok(AttackOutcome {
        state: AttackState {
            combo: wire.state.combo,
            back_to_back: wire.state.back_to_back,
        },
        packets: convert_packets(wire.packets, &format!("{path}.packets"))?,
        base_attack: wire.base_attack,
        clear_attack: wire.clear_attack,
        back_to_back_bonus: wire.back_to_back_bonus,
        special_bonus: wire.special_bonus,
        surge_attack: wire.surge_attack,
        perfect_clear_attack: wire.perfect_clear_attack,
        difficult: wire.difficult,
        back_to_back: wire.back_to_back,
    })
}

fn convert_cancellation(
    wire: WireGarbageCancellationOutcome,
    path: &str,
) -> Result<GarbageCancellationOutcome, NormalizedFixtureError> {
    Ok(GarbageCancellationOutcome {
        outgoing: convert_packets(wire.outgoing, &format!("{path}.outgoing"))?,
        attack_cancelled: wire.attack_cancelled,
        opener_bonus_cancelled: wire.opener_bonus_cancelled,
        sent_lines_after: wire.sent_lines_after,
    })
}

fn convert_packets(
    wire: Vec<WireAttackPacket>,
    path: &str,
) -> Result<AttackPackets, NormalizedFixtureError> {
    let packets = wire
        .into_iter()
        .enumerate()
        .map(|(index, packet)| {
            if packet.lines == 0 {
                return invalid(
                    format!("{path}[{index}].lines"),
                    "packet lines must be positive",
                );
            }
            Ok(AttackPacket {
                kind: parse_attack_packet_kind(&packet.kind, &format!("{path}[{index}].kind"))?,
                lines: packet.lines,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    AttackPackets::try_from_slice(&packets).map_err(|_| NormalizedFixtureError::InvalidField {
        field: path.to_owned(),
        reason: "attack packet capacity exceeded",
    })
}

fn convert_incoming(
    wire: WireIncomingGarbagePacket,
    path: &str,
) -> Result<IncomingGarbagePacket, NormalizedFixtureError> {
    if wire.lines == 0 {
        return invalid(format!("{path}.lines"), "packet lines must be positive");
    }
    if wire
        .hole_column
        .is_some_and(|column| usize::from(column) >= WIDTH)
    {
        return invalid(
            format!("{path}.hole_column"),
            "hole column must be within the board",
        );
    }
    Ok(IncomingGarbagePacket {
        lines: wire.lines,
        hole_column: wire.hole_column,
        ready_at_frame: wire.ready_at_frame,
        hardened: wire.hardened,
    })
}

fn convert_piece(wire: WirePieceState, path: &str) -> Result<PieceState, NormalizedFixtureError> {
    Ok(PieceState {
        kind: parse_piece_kind(&wire.kind, &format!("{path}.kind"))?,
        orientation: parse_orientation(&wire.orientation, &format!("{path}.orientation"))?,
        x: wire.x,
        y: wire.y,
    })
}

fn convert_timing(
    wire: WireTimingSnapshot,
    path: &str,
) -> Result<TimingSnapshot, NormalizedFixtureError> {
    if wire.fall_fraction_micros >= 1_000_000 {
        return invalid(
            format!("{path}.fall_fraction_micros"),
            "fraction must be below 1000000",
        );
    }
    Ok(TimingSnapshot {
        fall_fraction_micros: wire.fall_fraction_micros,
        lock_elapsed_frames: wire.lock_elapsed_frames,
        lock_resets_used: wire.lock_resets_used,
        locked: wire.locked,
        last_action: match wire.last_action {
            WireLastAction::None => LastAction::None,
            WireLastAction::Translation => LastAction::Translation,
            WireLastAction::SoftDrop => LastAction::SoftDrop,
            WireLastAction::HardDrop => LastAction::HardDrop,
            WireLastAction::Rotation {
                direction,
                kick_index,
            } => LastAction::Rotation {
                direction: parse_rotation_direction(
                    &direction,
                    &format!("{path}.last_action.direction"),
                )?,
                kick_index,
            },
        },
    })
}

fn convert_rows(rows: Vec<u16>, path: &str) -> Result<[u16; HEIGHT], NormalizedFixtureError> {
    if rows.len() != HEIGHT {
        return invalid(path.to_owned(), "row count must equal engine board height");
    }
    let valid_mask = (1_u16 << WIDTH) - 1;
    for (index, row) in rows.iter().copied().enumerate() {
        if row & !valid_mask != 0 {
            return invalid(
                format!("{path}[{index}]"),
                "row contains bits outside the board width",
            );
        }
    }
    rows.try_into()
        .map_err(|_| NormalizedFixtureError::InvalidField {
            field: path.to_owned(),
            reason: "row count must equal engine board height",
        })
}

fn validate_frames(trace: &[FrameSnapshot]) -> Result<(), NormalizedFixtureError> {
    if trace.is_empty() {
        return invalid(
            "snapshots".to_owned(),
            "trace must contain at least one frame",
        );
    }
    for (index, frames) in trace.windows(2).enumerate() {
        if frames[0].frame >= frames[1].frame {
            return invalid(
                format!("snapshots[{}].frame", index + 1),
                "frame numbers must be strictly increasing",
            );
        }
    }
    Ok(())
}

fn validate_battle_frames(trace: &[BattleSnapshot]) -> Result<(), NormalizedFixtureError> {
    if trace.is_empty() {
        return invalid(
            "snapshots".to_owned(),
            "trace must contain at least one frame",
        );
    }
    for (index, frames) in trace.windows(2).enumerate() {
        if frames[0].frame >= frames[1].frame {
            return invalid(
                format!("snapshots[{}].frame", index + 1),
                "frame numbers must be strictly increasing",
            );
        }
    }
    Ok(())
}

fn parse_case_kind(value: &str) -> Result<FunctionalCaseKind, NormalizedFixtureError> {
    match value {
        "boundary" => Ok(FunctionalCaseKind::Boundary),
        "randomized_battle" => Ok(FunctionalCaseKind::RandomizedBattle),
        _ => invalid("case_kind".to_owned(), "unknown case kind"),
    }
}

fn parse_claims(values: &[String]) -> Result<Vec<MechanicClaim>, NormalizedFixtureError> {
    if values.is_empty() {
        return invalid(
            "claims".to_owned(),
            "at least one mechanics claim is required",
        );
    }
    let mut seen = HashSet::new();
    let mut claims = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let claim =
            MechanicClaim::from_id(value).ok_or_else(|| NormalizedFixtureError::InvalidField {
                field: format!("claims[{index}]"),
                reason: "unknown mechanics claim",
            })?;
        if !seen.insert(claim) {
            return invalid(format!("claims[{index}]"), "duplicate mechanics claim");
        }
        claims.push(claim);
    }
    Ok(claims)
}

fn parse_piece_kind(value: &str, field: &str) -> Result<PieceKind, NormalizedFixtureError> {
    match value {
        "I" => Ok(PieceKind::I),
        "J" => Ok(PieceKind::J),
        "L" => Ok(PieceKind::L),
        "O" => Ok(PieceKind::O),
        "S" => Ok(PieceKind::S),
        "T" => Ok(PieceKind::T),
        "Z" => Ok(PieceKind::Z),
        _ => invalid(field.to_owned(), "unknown piece kind"),
    }
}

fn parse_orientation(value: &str, field: &str) -> Result<Orientation, NormalizedFixtureError> {
    match value {
        "spawn" => Ok(Orientation::Spawn),
        "right" => Ok(Orientation::Right),
        "reverse" => Ok(Orientation::Reverse),
        "left" => Ok(Orientation::Left),
        _ => invalid(field.to_owned(), "unknown orientation"),
    }
}

fn parse_rotation_direction(
    value: &str,
    field: &str,
) -> Result<RotationDirection, NormalizedFixtureError> {
    match value {
        "clockwise" => Ok(RotationDirection::Clockwise),
        "counterclockwise" => Ok(RotationDirection::Counterclockwise),
        "half" => Ok(RotationDirection::Half),
        _ => invalid(field.to_owned(), "unknown rotation direction"),
    }
}

fn parse_top_out(value: &str, field: &str) -> Result<TopOutReason, NormalizedFixtureError> {
    match value {
        "block_out" => Ok(TopOutReason::BlockOut),
        "lock_out" => Ok(TopOutReason::LockOut),
        "partial_lock_out" => Ok(TopOutReason::PartialLockOut),
        "garbage_out" => Ok(TopOutReason::GarbageOut),
        _ => invalid(field.to_owned(), "unknown top-out reason"),
    }
}

fn parse_attack_packet_kind(
    value: &str,
    field: &str,
) -> Result<AttackPacketKind, NormalizedFixtureError> {
    match value {
        "surge" => Ok(AttackPacketKind::Surge),
        "clear" => Ok(AttackPacketKind::Clear),
        "perfect_clear" => Ok(AttackPacketKind::PerfectClear),
        _ => invalid(field.to_owned(), "unknown attack packet kind"),
    }
}

fn parse_battle_result(value: &str, field: &str) -> Result<BattleResult, NormalizedFixtureError> {
    match value {
        "ongoing" => Ok(BattleResult::Ongoing),
        "player_one_win" => Ok(BattleResult::PlayerOneWin),
        "player_two_win" => Ok(BattleResult::PlayerTwoWin),
        "draw" => Ok(BattleResult::Draw),
        _ => invalid(field.to_owned(), "unknown battle result"),
    }
}

fn parse_bits(value: &str, field: &str) -> Result<u64, NormalizedFixtureError> {
    let Some(payload) = value.strip_prefix("0x") else {
        return invalid(field.to_owned(), "expected 0x followed by 16 hex digits");
    };
    if payload.len() != 16 || !payload.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid(field.to_owned(), "expected 0x followed by 16 hex digits");
    }
    u64::from_str_radix(payload, 16).map_err(|_| NormalizedFixtureError::InvalidField {
        field: field.to_owned(),
        reason: "invalid IEEE-754 payload",
    })
}

fn validate_nonempty(value: &str, field: &str) -> Result<(), NormalizedFixtureError> {
    if value.trim().is_empty() {
        return invalid(field.to_owned(), "value must not be blank");
    }
    Ok(())
}

fn require_sha256(value: &str, field: &'static str) -> Result<(), NormalizedFixtureError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(NormalizedFixtureError::InvalidSha256 { field });
    }
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn invalid<T>(field: String, reason: &'static str) -> Result<T, NormalizedFixtureError> {
    Err(NormalizedFixtureError::InvalidField { field, reason })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireManifest {
    schema_version: u16,
    case_id: String,
    case_kind: String,
    target_profile: String,
    reference_build: String,
    source: String,
    source_artifact_sha256: String,
    trace_sha256: String,
    claims: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "trace_kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireTrace {
    Solo {
        schema_version: u16,
        snapshots: Vec<WireFrameSnapshot>,
    },
    Battle {
        schema_version: u16,
        snapshots: Vec<WireBattleSnapshot>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFrameSnapshot {
    frame: u64,
    board_rows: Vec<u16>,
    garbage_rows: Vec<u16>,
    active: WirePieceState,
    hold: Option<String>,
    preview: Vec<String>,
    top_out: Option<String>,
    timing: Option<WireTimingSnapshot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePieceState {
    kind: String,
    orientation: String,
    x: i16,
    y: i16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTimingSnapshot {
    fall_fraction_micros: u32,
    lock_elapsed_frames: u16,
    lock_resets_used: u16,
    locked: bool,
    last_action: WireLastAction,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireLastAction {
    None,
    Translation,
    SoftDrop,
    HardDrop,
    Rotation { direction: String, kick_index: u8 },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBattleSnapshot {
    frame: u64,
    player_one: WireBattlePlayerSnapshot,
    player_two: WireBattlePlayerSnapshot,
    garbage_multiplier_bits: String,
    result: String,
    events: Option<WireBattleEventsSnapshot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBattlePlayerSnapshot {
    game: WireFrameSnapshot,
    attack: WireAttackState,
    incoming: Vec<WireIncomingGarbagePacket>,
    sent_lines: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAttackState {
    combo: u32,
    back_to_back: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireIncomingGarbagePacket {
    lines: u32,
    hole_column: Option<u8>,
    ready_at_frame: u64,
    hardened: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBattleEventsSnapshot {
    frame: u64,
    player_one: WireBattlePlayerEventsSnapshot,
    player_two: WireBattlePlayerEventsSnapshot,
    result: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBattlePlayerEventsSnapshot {
    attack: Option<WireAttackOutcome>,
    cancellation: Option<WireGarbageCancellationOutcome>,
    insertion: Option<WireGarbageInsertionOutcome>,
    transmitted: Vec<WireAttackPacket>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAttackOutcome {
    state: WireAttackState,
    packets: Vec<WireAttackPacket>,
    base_attack: u32,
    clear_attack: u32,
    back_to_back_bonus: u32,
    special_bonus: u32,
    surge_attack: u32,
    perfect_clear_attack: u32,
    difficult: bool,
    back_to_back: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAttackPacket {
    kind: String,
    lines: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireGarbageCancellationOutcome {
    outgoing: Vec<WireAttackPacket>,
    attack_cancelled: u32,
    opener_bonus_cancelled: u32,
    sent_lines_after: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireGarbageInsertionOutcome {
    inserted: u8,
    overflowed_buffer: bool,
    blocked_by_clear: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        LoadedReferenceTrace, NormalizedFixtureError, hex_sha256, load_normalized_fixture,
    };

    const TRACE: &str = r#"{
  "schema_version": 1,
  "trace_kind": "solo",
  "snapshots": [{
    "frame": 0,
    "board_rows": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    "garbage_rows": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    "active": {"kind":"I","orientation":"spawn","x":3,"y":20},
    "hold": null,
    "preview": ["J","L","O","S","T"],
    "top_out": null,
    "timing": null
  }]
}"#;

    fn manifest(trace_hash: &str, claim: &str) -> Vec<u8> {
        format!(
            r#"{{
  "schema_version": 1,
  "case_id": "schema-example-solo",
  "case_kind": "boundary",
  "target_profile": "schema-example-not-conformance",
  "reference_build": "schema-example",
  "source": "synthetic schema test only",
  "source_artifact_sha256": "{}",
  "trace_sha256": "{trace_hash}",
  "claims": ["{claim}"]
}}"#,
            "a".repeat(64)
        )
        .into_bytes()
    }

    #[test]
    fn valid_manifest_loads_exact_solo_snapshot() {
        let fixture = load_normalized_fixture(
            &manifest(&hex_sha256(TRACE.as_bytes()), "board_and_clear_geometry"),
            TRACE.as_bytes(),
        )
        .expect("valid normalized fixture");

        assert_eq!(fixture.trace_sha256, hex_sha256(TRACE.as_bytes()));
        let LoadedReferenceTrace::Solo(trace) = fixture.trace else {
            panic!("expected solo trace");
        };
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].active.x, 3);
    }

    #[test]
    fn changed_trace_bytes_fail_before_json_is_trusted() {
        let error = load_normalized_fixture(
            &manifest(&"0".repeat(64), "board_and_clear_geometry"),
            TRACE.as_bytes(),
        )
        .expect_err("hash mismatch");
        assert!(matches!(
            error,
            NormalizedFixtureError::TraceHashMismatch { .. }
        ));
    }

    #[test]
    fn battle_only_claim_cannot_hide_in_solo_trace() {
        let error = load_normalized_fixture(
            &manifest(
                &hex_sha256(TRACE.as_bytes()),
                "garbage_transit_and_cancellation",
            ),
            TRACE.as_bytes(),
        )
        .expect_err("wrong trace kind");
        assert!(matches!(
            error,
            NormalizedFixtureError::TraceKindMismatch { .. }
        ));
    }

    #[test]
    fn battle_trace_preserves_exact_multiplier_and_event_projection() {
        let solo: serde_json::Value = serde_json::from_str(TRACE).expect("solo JSON");
        let game = solo["snapshots"][0].clone();
        let player = serde_json::json!({
            "game": game,
            "attack": {"combo": 0, "back_to_back": 0},
            "incoming": [],
            "sent_lines": 0
        });
        let player_events = serde_json::json!({
            "attack": null,
            "cancellation": null,
            "insertion": null,
            "transmitted": []
        });
        let trace = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "trace_kind": "battle",
            "snapshots": [{
                "frame": 0,
                "player_one": player.clone(),
                "player_two": player,
                "garbage_multiplier_bits": "0x3ff0000000000000",
                "result": "ongoing",
                "events": {
                    "frame": 0,
                    "player_one": player_events.clone(),
                    "player_two": player_events,
                    "result": "ongoing"
                }
            }]
        }))
        .expect("battle trace JSON");
        let fixture = load_normalized_fixture(
            &manifest(&hex_sha256(&trace), "garbage_transit_and_cancellation"),
            &trace,
        )
        .expect("valid battle trace");

        let LoadedReferenceTrace::Battle(trace) = fixture.trace else {
            panic!("expected battle trace");
        };
        assert_eq!(
            trace[0].garbage_multiplier.ieee_bits(),
            0x3ff0_0000_0000_0000
        );
        assert_eq!(trace[0].events.expect("event projection").frame, 0);
    }

    #[test]
    fn unknown_json_fields_are_rejected() {
        let modified = TRACE.replace("\"frame\": 0,", "\"frame\": 0, \"guess\": true,");
        let error = load_normalized_fixture(
            &manifest(&hex_sha256(modified.as_bytes()), "board_and_clear_geometry"),
            modified.as_bytes(),
        )
        .expect_err("unknown field");
        assert!(matches!(error, NormalizedFixtureError::Json { .. }));
    }

    #[test]
    fn committed_schema_example_hash_and_shape_stay_valid() {
        let fixture = load_normalized_fixture(
            include_bytes!(
                "../../../fixtures/conformance/examples/schema-example-solo-v1.manifest.json"
            ),
            include_bytes!(
                "../../../fixtures/conformance/examples/schema-example-solo-v1.trace.json"
            ),
        )
        .expect("committed schema example");

        assert_eq!(fixture.id, "schema-example-solo-v1");
        assert_eq!(fixture.trace.len(), 1);
    }
}
