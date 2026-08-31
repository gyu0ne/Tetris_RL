param(
    [ValidateSet('light', 'balanced', 'max')]
    [string]$ResourceProfile = 'max',
    [double]$HoursPerStage = 4,
    [string]$OutputDir = 'checkpoints/versus-selfplay-r6',
    [int]$ProbeSeeds = 8,
    [int]$ProbeHorizon = 1000,
    [double]$MinimumAttackRatioAt100 = 1.10,
    [int]$SelectionSeeds = 8,
    [int]$SelectionHorizon = 2000,
    [switch]$SkipProbes,
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
if ($MinimumAttackRatioAt100 -le 0) { throw 'MinimumAttackRatioAt100 must be positive.' }
if ($SelectionSeeds -le 0 -or $SelectionHorizon -le 0) {
    throw 'Selection counts must be positive.'
}

function Assert-ProbeIdentity {
    param(
        [string]$Path,
        [string]$Candidate,
        [string]$Opponent
    )
    if (-not (Test-Path $Path)) { return }
    $Report = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if (
        $Report.schema_version -ne 'paired-versus-evaluation-v2' -or
        $Report.candidate -ne $Candidate -or
        $Report.opponent -ne $Opponent -or
        $Report.base_seed -ne 880001 -or
        $Report.seeds -ne $ProbeSeeds -or
        $Report.horizon -ne $ProbeHorizon -or
        $Report.frames_per_placement -ne 12
    ) {
        throw "probe parameters changed; choose a new OutputDir instead of reusing $Path"
    }
}

$dirtyFiles = @(git status --porcelain)
if ($LASTEXITCODE -ne 0) { throw 'git status failed.' }
if ($dirtyFiles.Count -ne 0) {
    throw 'Commit or stash the working tree before starting a retained training run.'
}

$R4Selected = 'checkpoints/versus-selfplay-r4/selected-model.pt'
$ProbeAnchor = 'checkpoints/versus-selfplay-r3/snapshots/update-000700-model.pt'
foreach ($required in @(
    $R4Selected,
    $ProbeAnchor,
    'checkpoints/versus-selfplay-r3/snapshots/update-001050-model.pt',
    'checkpoints/solo-imitation-versus-bootstrap-v1/model.pt'
)) {
    if (-not (Test-Path $required)) { throw "required checkpoint not found: $required" }
}

docker compose build training
if ($LASTEXITCODE -ne 0) { throw 'training image build failed.' }

$ProbeDir = "$OutputDir/probes"
$BaselineProbe = "$ProbeDir/r4-baseline.json"
Assert-ProbeIdentity -Path $BaselineProbe -Candidate $R4Selected -Opponent $ProbeAnchor
if (-not $SkipProbes -and -not (Test-Path $BaselineProbe)) {
    Write-Host 'Measuring the fixed r4 attack baseline...'
    docker compose run --rm -e "RAYON_NUM_THREADS=$RayonThreads" training `
        python -m tetris_rl.evaluation.versus `
        --candidate $R4Selected `
        --opponent $ProbeAnchor `
        --output $BaselineProbe `
        --base-seed 880001 `
        --seeds $ProbeSeeds `
        --horizon $ProbeHorizon `
        --frames-per-placement 12 `
        --threads $TorchThreads `
        --allow-observed
    if ($LASTEXITCODE -ne 0) { throw 'r4 attack baseline probe failed.' }
}

$Stages = @(50, 100, 150, 200)
foreach ($Stage in $Stages) {
    $Snapshot = "$OutputDir/snapshots/update-$($Stage.ToString('000000'))-model.pt"
    if (-not (Test-Path $Snapshot)) {
        $TrainingArguments = @(
            'compose', 'run', '--rm', '-e', "RAYON_NUM_THREADS=$RayonThreads", 'training',
            'python', '-m', 'tetris_rl.training.selfplay',
            '--config', 'configs/training/versus_selfplay_ppo_v7.json',
            '--bootstrap', 'checkpoints/solo-imitation-versus-bootstrap-v1/model.pt',
            '--output-dir', $OutputDir,
            '--hours', "$HoursPerStage",
            '--max-updates', "$Stage",
            '--threads', "$TorchThreads",
            '--allow-observed'
        )
        if (Test-Path "$OutputDir/latest.pt") {
            $TrainingArguments += '--resume'
        } else {
            $TrainingArguments += @('--initialize-from', $R4Selected)
        }
        Write-Host "Training offense r6 through cumulative update $Stage..."
        docker @TrainingArguments
        if ($LASTEXITCODE -ne 0) { throw "r6 training failed before update $Stage." }
        if (-not (Test-Path $Snapshot)) {
            throw "stage time limit expired before required snapshot $Snapshot was produced."
        }
    }

    if (-not $SkipProbes) {
        $CandidateProbe = "$ProbeDir/update-$($Stage.ToString('000000')).json"
        Assert-ProbeIdentity -Path $CandidateProbe -Candidate $Snapshot -Opponent $ProbeAnchor
        if (-not (Test-Path $CandidateProbe)) {
            Write-Host "Probing attack strength at update $Stage..."
            docker compose run --rm -e "RAYON_NUM_THREADS=$RayonThreads" training `
                python -m tetris_rl.evaluation.versus `
                --candidate $Snapshot `
                --opponent $ProbeAnchor `
                --output $CandidateProbe `
                --base-seed 880001 `
                --seeds $ProbeSeeds `
                --horizon $ProbeHorizon `
                --frames-per-placement 12 `
                --threads $TorchThreads `
                --allow-observed
            if ($LASTEXITCODE -ne 0) { throw "r6 update $Stage probe failed." }
        }
        $Baseline = (Get-Content -LiteralPath $BaselineProbe -Raw | ConvertFrom-Json).combined
        $Candidate = (Get-Content -LiteralPath $CandidateProbe -Raw | ConvertFrom-Json).combined
        $AttackRatio = if ($Baseline.outgoing_attack_per_piece -gt 0) {
            $Candidate.outgoing_attack_per_piece / $Baseline.outgoing_attack_per_piece
        } else { 0.0 }
        [pscustomobject]@{
            event = 'offense_stage_probe'
            update = $Stage
            attack_ratio = $AttackRatio
            score = $Candidate.score
            outgoing_attack_per_piece = $Candidate.outgoing_attack_per_piece
            danger_rate = $Candidate.danger_rate
            mean_holes = $Candidate.mean_holes
        } | ConvertTo-Json -Compress | Write-Host
        if ($Stage -eq 100 -and $AttackRatio -lt $MinimumAttackRatioAt100) {
            $StopReport = [pscustomobject]@{
                schema_version = 'offense-stage-stop-v1'
                stopped_after_update = $Stage
                attack_ratio = $AttackRatio
                required_attack_ratio = $MinimumAttackRatioAt100
                reason = 'attack improvement gate failed'
            }
            $StopReport | ConvertTo-Json | Set-Content -LiteralPath "$ProbeDir/early-stop.json" -Encoding utf8
            Write-Warning "r6 stopped at update 100: attack ratio $AttackRatio is below $MinimumAttackRatioAt100."
            return
        }
    }
}

if (-not $SkipSelection) {
    Write-Host 'Applying final win, attack and stability promotion gates...'
    docker compose run --rm -e "RAYON_NUM_THREADS=$RayonThreads" training `
        python -m tetris_rl.evaluation.versus_select `
        --output-dir $OutputDir `
        --anchor checkpoints/versus-selfplay-r3/snapshots/update-000700-model.pt `
        --anchor checkpoints/versus-selfplay-r3/snapshots/update-001050-model.pt `
        --baseline $R4Selected `
        --min-score-delta -0.03 `
        --min-direct-baseline-score 0.47 `
        --min-attack-ratio 1.20 `
        --max-danger-ratio 1.15 `
        --max-holes-ratio 1.15 `
        --seeds $SelectionSeeds `
        --horizon $SelectionHorizon `
        --cadences 8,12,15 `
        --threads $TorchThreads `
        --allow-observed
    if ($LASTEXITCODE -ne 0) { throw 'r6 gated champion selection failed.' }
}
