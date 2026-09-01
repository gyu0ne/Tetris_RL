param(
    [ValidateSet('light', 'balanced', 'max')]
    [string]$ResourceProfile = 'max',
    [double]$HoursPerStage = 4,
    [string]$OutputDir = 'checkpoints/versus-selfplay-r8',
    [int]$ProbeSeeds = 16,
    [int]$ProbeHorizon = 2000,
    [int]$SelectionSeeds = 32,
    [int]$SelectionHorizon = 4000,
    [switch]$SkipSelection
)

$ErrorActionPreference = 'Stop'

switch ($ResourceProfile) {
    'light' { $RayonThreads = 2; $TorchThreads = 1 }
    'balanced' { $RayonThreads = 6; $TorchThreads = 2 }
    'max' { $RayonThreads = 12; $TorchThreads = 2 }
}
if ($HoursPerStage -le 0) { throw 'HoursPerStage must be positive.' }
if ($ProbeSeeds -le 0 -or $ProbeHorizon -le 0) { throw 'Probe counts must be positive.' }
if ($SelectionSeeds -le 0 -or $SelectionHorizon -le 0) {
    throw 'Selection counts must be positive.'
}

$R4Selected = 'checkpoints/versus-selfplay-r4/selected-model.pt'
$R7Aggressive = 'checkpoints/versus-selfplay-r7/aggressive-model.pt'
$HardAnchor = 'checkpoints/versus-selfplay-r3/snapshots/update-000700-model.pt'
$LateAnchor = 'checkpoints/versus-selfplay-r3/snapshots/update-001050-model.pt'
$Bootstrap = 'checkpoints/solo-imitation-versus-bootstrap-v1/model.pt'
$Config = 'configs/training/versus_selfplay_ppo_r8.json'
$ProbeBaseSeed = 2080001

foreach ($required in @($R4Selected, $R7Aggressive, $HardAnchor, $LateAnchor, $Bootstrap, $Config)) {
    if (-not (Test-Path -LiteralPath $required)) { throw "required input not found: $required" }
}

$dirtyFiles = @(git status --porcelain)
if ($LASTEXITCODE -ne 0) { throw 'git status failed.' }
if ($dirtyFiles.Count -ne 0) {
    throw 'Commit or stash the working tree before starting a retained training run.'
}

function Assert-EvaluationIdentity {
    param(
        [string]$Path,
        [string]$Candidate,
        [string]$Opponent
    )
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $Report = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if (
        $Report.schema_version -ne 'paired-versus-evaluation-v2' -or
        $Report.candidate -ne $Candidate -or
        $Report.opponent -ne $Opponent -or
        $Report.base_seed -ne $ProbeBaseSeed -or
        $Report.seeds -ne $ProbeSeeds -or
        $Report.horizon -ne $ProbeHorizon -or
        $Report.frames_per_placement -ne 12
    ) {
        throw "evaluation parameters changed; choose a new OutputDir instead of reusing $Path"
    }
}

function Invoke-FixedEvaluation {
    param(
        [string]$Candidate,
        [string]$Opponent,
        [string]$Output
    )
    Assert-EvaluationIdentity -Path $Output -Candidate $Candidate -Opponent $Opponent
    if (Test-Path -LiteralPath $Output) { return }
    docker compose run --rm -e "RAYON_NUM_THREADS=$RayonThreads" training `
        python -m tetris_rl.evaluation.versus `
        --candidate $Candidate `
        --opponent $Opponent `
        --output $Output `
        --base-seed $ProbeBaseSeed `
        --seeds $ProbeSeeds `
        --horizon $ProbeHorizon `
        --frames-per-placement 12 `
        --threads $TorchThreads `
        --allow-observed
    if ($LASTEXITCODE -ne 0) { throw "fixed evaluation failed: $Candidate versus $Opponent" }
}

docker compose build training
if ($LASTEXITCODE -ne 0) { throw 'training image build failed.' }

$ProbeDir = "$OutputDir/probes"
$ReferenceHard = "$ProbeDir/reference-r7-vs-r3-700.json"
$ReferenceR4 = "$ProbeDir/reference-r7-vs-r4.json"
Invoke-FixedEvaluation -Candidate $R7Aggressive -Opponent $HardAnchor -Output $ReferenceHard
Invoke-FixedEvaluation -Candidate $R7Aggressive -Opponent $R4Selected -Output $ReferenceR4

$ReferenceHardMetrics = (Get-Content -LiteralPath $ReferenceHard -Raw | ConvertFrom-Json).combined
$ReferenceR4Metrics = (Get-Content -LiteralPath $ReferenceR4 -Raw | ConvertFrom-Json).combined
$Stages = @(25, 50, 75, 100, 125, 150)

foreach ($Stage in $Stages) {
    $Snapshot = "$OutputDir/snapshots/update-$($Stage.ToString('000000'))-model.pt"
    if (-not (Test-Path -LiteralPath $Snapshot)) {
        $TrainingArguments = @(
            'compose', 'run', '--rm', '-e', "RAYON_NUM_THREADS=$RayonThreads", 'training',
            'python', '-m', 'tetris_rl.training.selfplay',
            '--config', $Config,
            '--bootstrap', $Bootstrap,
            '--output-dir', $OutputDir,
            '--hours', "$HoursPerStage",
            '--max-updates', "$Stage",
            '--threads', "$TorchThreads",
            '--allow-observed'
        )
        if (Test-Path -LiteralPath "$OutputDir/latest.pt") {
            $TrainingArguments += '--resume'
        } else {
            $TrainingArguments += @('--initialize-from', $R7Aggressive)
        }
        Write-Host "Training r8 through cumulative update $Stage..."
        docker @TrainingArguments
        if ($LASTEXITCODE -ne 0) { throw "r8 training failed before update $Stage." }
        if (-not (Test-Path -LiteralPath $Snapshot)) {
            throw "stage time limit expired before required snapshot $Snapshot was produced."
        }
    }

    $HardProbe = "$ProbeDir/update-$($Stage.ToString('000000'))-vs-r3-700.json"
    $R4Probe = "$ProbeDir/update-$($Stage.ToString('000000'))-vs-r4.json"
    Invoke-FixedEvaluation -Candidate $Snapshot -Opponent $HardAnchor -Output $HardProbe
    Invoke-FixedEvaluation -Candidate $Snapshot -Opponent $R4Selected -Output $R4Probe

    $HardMetrics = (Get-Content -LiteralPath $HardProbe -Raw | ConvertFrom-Json).combined
    $R4Metrics = (Get-Content -LiteralPath $R4Probe -Raw | ConvertFrom-Json).combined
    $AttackRetention = if ($ReferenceHardMetrics.outgoing_attack_per_piece -gt 0) {
        $HardMetrics.outgoing_attack_per_piece / $ReferenceHardMetrics.outgoing_attack_per_piece
    } else { 0.0 }
    [pscustomobject]@{
        event = 'r8_stage_probe'
        update = $Stage
        attack_retention = $AttackRetention
        hard_anchor_score = $HardMetrics.score
        hard_anchor_score_delta = $HardMetrics.score - $ReferenceHardMetrics.score
        direct_r4_score = $R4Metrics.score
        direct_r4_score_delta = $R4Metrics.score - $ReferenceR4Metrics.score
        outgoing_attack_per_piece = $HardMetrics.outgoing_attack_per_piece
        danger_rate = $HardMetrics.danger_rate
        mean_holes = $HardMetrics.mean_holes
    } | ConvertTo-Json -Compress | Write-Host
}

if (-not $SkipSelection) {
    Write-Host 'Selecting a 12-frame offense-preserving r8 checkpoint...'
    docker compose run --rm -e "RAYON_NUM_THREADS=$RayonThreads" training `
        python -m tetris_rl.evaluation.versus_select `
        --output-dir $OutputDir `
        --anchor $HardAnchor `
        --anchor $LateAnchor `
        --baseline $R4Selected `
        --min-score-delta -0.02 `
        --min-direct-baseline-score 0.50 `
        --min-attack-ratio 1.05 `
        --max-danger-ratio 1.50 `
        --max-holes-ratio 1.25 `
        --shortlist 6 `
        --seeds $SelectionSeeds `
        --horizon $SelectionHorizon `
        --cadences 12 `
        --base-seed 2980001 `
        --threads $TorchThreads `
        --allow-observed
    if ($LASTEXITCODE -ne 0) { throw 'r8 target-cadence selection failed.' }
}
