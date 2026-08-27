param(
    [ValidateSet('light', 'balanced', 'max')]
    [string]$ResourceProfile = 'balanced',
    [double]$Hours = 24,
    [int]$MaxUpdates = 0,
    [string]$InitializeFrom = '',
    [string]$OutputDir = 'checkpoints/versus-selfplay-r1',
    [switch]$Resume
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

switch ($ResourceProfile) {
    'light' { $Threads = 2 }
    'balanced' { $Threads = 6 }
    'max' { $Threads = 12 }
}
if ($Hours -le 0) { throw 'Hours must be positive.' }
if ($MaxUpdates -lt 0) { throw 'MaxUpdates must be nonnegative.' }

$dirtyFiles = @(git status --porcelain)
if ($LASTEXITCODE -ne 0) { throw 'git status failed.' }
if ($dirtyFiles.Count -ne 0) {
    throw 'Self-play must start from a clean commit so checkpoint provenance is exact.'
}

docker compose build training
if ($LASTEXITCODE -ne 0) { throw 'training image build failed.' }

if (-not $InitializeFrom) {
    $PreferredInitialization = 'checkpoints/versus-selfplay-r0/snapshots/update-000050-model.pt'
    if (Test-Path -LiteralPath $PreferredInitialization -PathType Leaf) {
        $InitializeFrom = $PreferredInitialization
    }
}

$arguments = @(
    'compose', 'run', '--rm', '-e', "RAYON_NUM_THREADS=$Threads", 'training',
    'python', '-m', 'tetris_rl.training.selfplay',
    '--config', 'configs/training/versus_selfplay_ppo_v2.json',
    '--bootstrap', 'checkpoints/solo-imitation-versus-bootstrap-v1/model.pt',
    '--output-dir', $OutputDir,
    '--hours', "$Hours",
    '--threads', "$Threads",
    '--allow-observed'
)
if ($MaxUpdates -gt 0) { $arguments += @('--max-updates', "$MaxUpdates") }
if ($Resume) {
    $arguments += '--resume'
} elseif ($InitializeFrom) {
    if (-not (Test-Path -LiteralPath $InitializeFrom -PathType Leaf)) {
        throw "Initialization checkpoint not found: $InitializeFrom"
    }
    $arguments += @('--initialize-from', $InitializeFrom)
}

Write-Host "Self-play v2: $ResourceProfile ($Threads threads, $Hours hours, output $OutputDir)"
docker @arguments
if ($LASTEXITCODE -ne 0) { throw "self-play failed with exit code $LASTEXITCODE" }
