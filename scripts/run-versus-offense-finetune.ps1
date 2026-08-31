param(
    [ValidateSet('light', 'balanced', 'max')]
    [string]$ResourceProfile = 'max',
    [double]$Hours = 12,
    [int]$MaxUpdates = 400,
    [string]$R4OutputDir = 'checkpoints/versus-selfplay-r4',
    [string]$OutputDir = 'checkpoints/versus-selfplay-r5',
    [int]$SelectionSeeds = 8,
    [int]$SelectionHorizon = 2000,
    [switch]$Resume,
    [switch]$SkipSelection
)

$ErrorActionPreference = 'Stop'

switch ($ResourceProfile) {
    'light' { $RayonThreads = 2; $TorchThreads = 1 }
    'balanced' { $RayonThreads = 6; $TorchThreads = 2 }
    'max' { $RayonThreads = 12; $TorchThreads = 2 }
}
if ($Hours -le 0 -or $MaxUpdates -le 0) { throw 'Hours and MaxUpdates must be positive.' }
if ($SelectionSeeds -le 0 -or $SelectionHorizon -le 0) {
    throw 'Selection seeds and horizon must be positive.'
}

$dirtyFiles = @(git status --porcelain)
if ($LASTEXITCODE -ne 0) { throw 'git status failed.' }
if ($dirtyFiles.Count -ne 0) {
    throw 'Commit or stash the working tree before starting a retained training run.'
}

docker compose build training
if ($LASTEXITCODE -ne 0) { throw 'training image build failed.' }

$R4Selected = "$R4OutputDir/selected-model.pt"
if (-not (Test-Path $R4Selected)) {
    Write-Host 'Selecting the robust r4 initialization before offense fine-tuning...'
    docker compose run --rm -e "RAYON_NUM_THREADS=$RayonThreads" training `
        python -m tetris_rl.evaluation.versus_select `
        --output-dir $R4OutputDir `
        --anchor checkpoints/versus-selfplay-r3/snapshots/update-000700-model.pt `
        --anchor checkpoints/versus-selfplay-r3/snapshots/update-001050-model.pt `
        --seeds $SelectionSeeds `
        --horizon $SelectionHorizon `
        --cadences 8,12,15 `
        --threads $TorchThreads `
        --allow-observed
    if ($LASTEXITCODE -ne 0) { throw 'r4 initialization selection failed.' }
}

$arguments = @(
    'compose', 'run', '--rm', '-e', "RAYON_NUM_THREADS=$RayonThreads", 'training',
    'python', '-m', 'tetris_rl.training.selfplay',
    '--config', 'configs/training/versus_selfplay_ppo_v6.json',
    '--bootstrap', 'checkpoints/solo-imitation-versus-bootstrap-v1/model.pt',
    '--output-dir', $OutputDir,
    '--hours', "$Hours",
    '--max-updates', "$MaxUpdates",
    '--threads', "$TorchThreads",
    '--allow-observed'
)
if ($Resume) {
    $arguments += '--resume'
} else {
    $arguments += @('--initialize-from', $R4Selected)
}

Write-Host "Offense fine-tune r5: $ResourceProfile (up to $MaxUpdates updates, output $OutputDir)"
docker @arguments
if ($LASTEXITCODE -ne 0) { throw "offense fine-tuning failed with exit code $LASTEXITCODE" }

if (-not $SkipSelection) {
    Write-Host 'Applying win, attack and stability promotion gates...'
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
    if ($LASTEXITCODE -ne 0) { throw 'r5 gated champion selection failed.' }
}
