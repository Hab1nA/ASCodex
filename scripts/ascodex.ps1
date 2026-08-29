[CmdletBinding()]
param(
    [ValidateSet('check', 'build', 'test', 'run')]
    [string]$Action = 'check',
    [switch]$SolverMode,
    [string]$PolicyFile,
    [string]$LedgerFile,
    [string]$WorkspaceRoot,
    [string]$ContractFile,
    [string]$ContractInputFile,
    [string]$CycleId,
    [string]$CycleEventVersion,
    [string]$CampaignId,
    [string]$ChallengeId,
    [string]$RecoveryId,
    [string]$RuntimeInstanceId,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CommandArgs
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$rustRoot = Join-Path $repoRoot 'codex/codex-rs'
$cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
if (-not $cargo) {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
}
if (-not $cargo) {
    throw 'Rust cargo was not found. Install Rust or add cargo to PATH.'
}

# The project's pinned 1.95 toolchain has intermittently suspended even on ordinary
# dependency compiles on this machine. The stable MSVC toolchain has repeatedly completed
# the same verification. Prefer it explicitly, while allowing the caller to override RUSTC.
$stableRustc = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\rustc.exe"
$stableCargo = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe"
if (Test-Path -LiteralPath $stableRustc -PathType Leaf) {
    $env:RUSTC = $stableRustc
}
if (Test-Path -LiteralPath $stableCargo -PathType Leaf) {
    $cargo = [System.IO.FileInfo]$stableCargo
}

# Keep large build artifacts and their heavy temp file churn out of the knowledge mirror.
if (-not (Test-Path Env:CARGO_TARGET_DIR)) {
    $env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA 'Temp\ascodex-cargo-target'
}

Push-Location $rustRoot
try {
    if ($SolverMode) {
        if ([string]::IsNullOrWhiteSpace($PolicyFile)) {
            throw '-SolverMode requires -PolicyFile pointing to a local typed policy.'
        }
        if ([string]::IsNullOrWhiteSpace($LedgerFile)) {
            throw '-SolverMode requires -LedgerFile pointing to a persistent local SQLite ledger.'
        }
        if ([string]::IsNullOrWhiteSpace($CycleId) -or [string]::IsNullOrWhiteSpace($CycleEventVersion) -or [string]::IsNullOrWhiteSpace($CampaignId) -or [string]::IsNullOrWhiteSpace($ChallengeId)) {
            throw '-SolverMode requires -CycleId, -CycleEventVersion, -CampaignId, and -ChallengeId from a Chief-issued ledger record.'
        }
        if ([string]::IsNullOrWhiteSpace($RecoveryId) -or [string]::IsNullOrWhiteSpace($RuntimeInstanceId)) {
            throw '-SolverMode requires -RecoveryId and -RuntimeInstanceId for the current process recovery canary.'
        }
        [UInt64]$parsedCycleEventVersion = 0
        if (-not [UInt64]::TryParse($CycleEventVersion, [Globalization.NumberStyles]::None, [Globalization.CultureInfo]::InvariantCulture, [ref]$parsedCycleEventVersion) -or $parsedCycleEventVersion -eq 0) {
            throw '-CycleEventVersion must be a nonzero unsigned integer from the Chief-issued ledger record.'
        }
        $policy = Resolve-Path -LiteralPath $PolicyFile -ErrorAction Stop
        if (-not [IO.Path]::IsPathRooted($policy.Path)) {
            throw 'Guard policy path must be absolute.'
        }
        if ($policy.Path -like '*ascodex-solver-policy.example.yaml') {
            throw 'The example policy is schema-only and cannot enable solver mode.'
        }
        $ledger = [IO.Path]::GetFullPath($LedgerFile)
        if (-not [IO.Path]::IsPathRooted($ledger)) {
            throw 'Guard ledger path must be absolute.'
        }
        if (-not (Test-Path -LiteralPath $ledger -PathType Leaf)) {
            throw 'Solver mode requires an existing Chief-issued SQLite ledger.'
        }
        if ([string]::IsNullOrWhiteSpace($ContractFile) -or [string]::IsNullOrWhiteSpace($ContractInputFile)) {
            throw '-SolverMode requires -ContractFile and -ContractInputFile from ascodex_contract.py.'
        }
        $contract = [IO.Path]::GetFullPath($ContractFile)
        $contractInput = [IO.Path]::GetFullPath($ContractInputFile)
        if (-not [IO.Path]::IsPathRooted($contract) -or -not (Test-Path -LiteralPath $contract -PathType Leaf)) {
            throw '-ContractFile must be an existing absolute typed ChallengeContract JSON file.'
        }
        if (-not [IO.Path]::IsPathRooted($contractInput) -or -not (Test-Path -LiteralPath $contractInput -PathType Leaf)) {
            throw '-ContractInputFile must be an existing absolute canonical fingerprint input file.'
        }
        $env:ASCODEX_SOLVER_MODE = '1'
        $env:ASCODEX_SOLVER_POLICY_FILE = $policy.Path
        $env:ASCODEX_SOLVER_LEDGER_FILE = $ledger
        $env:ASCODEX_CYCLE_ID = $CycleId
        $env:ASCODEX_CYCLE_EVENT_VERSION = $parsedCycleEventVersion.ToString([Globalization.CultureInfo]::InvariantCulture)
        $env:ASCODEX_CAMPAIGN_ID = $CampaignId
        $env:ASCODEX_CHALLENGE_ID = $ChallengeId
        $env:ASCODEX_RECOVERY_ID = $RecoveryId
        $env:ASCODEX_RUNTIME_INSTANCE_ID = $RuntimeInstanceId
        if (-not [string]::IsNullOrWhiteSpace($WorkspaceRoot)) {
            $resolvedWorkspace = Resolve-Path -LiteralPath $WorkspaceRoot -ErrorAction Stop
            $env:ASCODEX_SOLVER_WORKSPACE_ROOT = $resolvedWorkspace.Path
        }
        $env:ASCODEX_CONTRACT_FILE = $contract
        $env:ASCODEX_CONTRACT_INPUT_FILE = $contractInput
    } else {
        Remove-Item Env:ASCODEX_SOLVER_MODE -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_SOLVER_POLICY_FILE -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_SOLVER_LEDGER_FILE -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_CYCLE_ID -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_CYCLE_EVENT_VERSION -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_CAMPAIGN_ID -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_CHALLENGE_ID -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_RECOVERY_ID -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_RUNTIME_INSTANCE_ID -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_SOLVER_WORKSPACE_ROOT -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_CONTRACT_FILE -ErrorAction SilentlyContinue
        Remove-Item Env:ASCODEX_CONTRACT_INPUT_FILE -ErrorAction SilentlyContinue
    }

    switch ($Action) {
        'check' {
            & $cargo.Source check -p codex-core -p codex-app-server -p codex-solver-guard --locked --offline
        }
        'build' {
            & $cargo.Source build -p codex-cli --bin codex --locked --offline
        }
        'test' {
            & $cargo.Source test -p codex-ascodex-coordination -p codex-ascodex-runtime -p codex-solver-guard --locked --offline
        }
        'run' {
            & $cargo.Source run -p codex-cli --bin codex --locked --offline -- @CommandArgs
        }
    }
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}
