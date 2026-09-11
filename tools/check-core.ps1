# Uses the project-local toolchains installed for this Windows workspace.
# No download, system PATH change, or machine-wide setup is performed here.
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskPrevious = @{}
foreach ($key in @('CARGO_HOME', 'RUSTUP_HOME', 'CARGO_ENCODED_RUSTFLAGS')) {
    $taskPrevious[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
}
Push-Location $taskRoot
try {
    $env:CARGO_HOME = Join-Path $taskRoot '.tools/cargo'
    $env:RUSTUP_HOME = Join-Path $taskRoot '.tools/rustup'
    $taskCargo = Join-Path $env:CARGO_HOME 'bin/cargo.exe'
    $taskLlvm = Join-Path $env:RUSTUP_HOME 'toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin'
    $taskDlltool = Join-Path $taskLlvm 'llvm-dlltool.exe'
    if (!(Test-Path -LiteralPath $taskDlltool)) {
        # LLVM's llvm-ar is a multicall binary; the filename selects dlltool mode.
        Copy-Item -LiteralPath (Join-Path $taskLlvm 'llvm-ar.exe') -Destination $taskDlltool
    }
    $env:CARGO_ENCODED_RUSTFLAGS = '-C' + [char]31 + 'dlltool=' + $taskDlltool
    & $taskCargo fmt --all -- --check
    if ($LASTEXITCODE) { throw 'Formatting failed' }
    & $taskCargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets --offline --locked -- -D warnings
    if ($LASTEXITCODE) { throw 'Clippy failed' }
    & $taskCargo +stable-x86_64-pc-windows-gnu test --workspace --offline --locked
    if ($LASTEXITCODE) { throw 'Tests failed' }
    & $taskCargo +stable-x86_64-pc-windows-gnu run -p vocal-time --example tempo_map --offline --locked
    if ($LASTEXITCODE) { throw 'Tempo example failed' }
    & $taskCargo +stable-x86_64-pc-windows-gnu run -p vocal-project --example project_roundtrip --offline --locked
    if ($LASTEXITCODE) { throw 'Project example failed' }
    & $taskCargo +stable-x86_64-pc-windows-gnu run -p vocal-project --example import_wav --offline --locked
    if ($LASTEXITCODE) { throw 'WAV example failed' }
    & $taskCargo +stable-x86_64-pc-windows-gnu run -p vocal-analysis-runtime --example analyze_waveform --offline --locked
    if ($LASTEXITCODE) { throw 'Analysis example failed' }
    & $taskCargo +stable-x86_64-pc-windows-gnu run -p vocal-app --example import_session --offline --locked
    if ($LASTEXITCODE) { throw 'App session example failed' }
}
finally {
    Pop-Location
    foreach ($key in $taskPrevious.Keys) {
        [Environment]::SetEnvironmentVariable($key, $taskPrevious[$key], 'Process')
    }
}
