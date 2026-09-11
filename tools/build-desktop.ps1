# Build with the isolated SDK when present; otherwise use a configured MSVC developer shell.
# Install JS dependencies with `npm ci` in apps/desktop before running this script.
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskKeys = @('CARGO_HOME','RUSTUP_HOME','CARGO_ENCODED_RUSTFLAGS','PATH','LIB','INCLUDE','RC')
$taskPrevious = @{}
foreach ($taskKey in $taskKeys) { $taskPrevious[$taskKey] = [Environment]::GetEnvironmentVariable($taskKey, 'Process') }
Push-Location $taskRoot
try {
    $env:CARGO_HOME = Join-Path $taskRoot '.tools/cargo'
    $env:RUSTUP_HOME = Join-Path $taskRoot '.tools/rustup'
    $env:CARGO_ENCODED_RUSTFLAGS = $null
    $taskCargo = Join-Path $env:CARGO_HOME 'bin/cargo.exe'
    $taskSdk = Join-Path $taskRoot '.tools/windows-sdk/microsoft.windows.sdk.cpp/c'
    if (Test-Path -LiteralPath $taskSdk) {
        $taskVs = 'C:/Program Files/Microsoft Visual Studio/18/Community/VC/Tools/MSVC/14.51.36231'
        if (!(Test-Path -LiteralPath "$taskVs/bin/Hostx64/x64/link.exe")) { throw 'MSVC C++ tools not found; use a configured developer shell or update the local tool path.' }
        $taskSdkLib = Join-Path $taskRoot '.tools/windows-sdk/microsoft.windows.sdk.cpp.x64/c'
        $env:PATH = "$taskVs/bin/Hostx64/x64;$taskSdk/bin/10.0.28000.0/x64;$env:PATH"
        $env:LIB = "$taskVs/lib/x64;$taskSdkLib/um/x64;$taskSdkLib/ucrt/x64"
        $env:INCLUDE = "$taskVs/include;$taskSdk/Include/10.0.28000.0/ucrt;$taskSdk/Include/10.0.28000.0/shared;$taskSdk/Include/10.0.28000.0/um;$taskSdk/Include/10.0.28000.0/winrt"
        $env:RC = "$taskSdk/bin/10.0.28000.0/x64/rc.exe"
    }
    Push-Location apps/desktop
    try {
        & npm.cmd run build
        if ($LASTEXITCODE) { throw 'Frontend build failed' }
    } finally { Pop-Location }
    & $taskCargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
    if ($LASTEXITCODE) { throw 'Desktop formatting failed' }
    & $taskCargo +stable-x86_64-pc-windows-msvc clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --features tauri/custom-protocol --offline --locked -- -D warnings
    if ($LASTEXITCODE) { throw 'Desktop Clippy failed' }
    & $taskCargo +stable-x86_64-pc-windows-msvc test --manifest-path apps/desktop/src-tauri/Cargo.toml --features tauri/custom-protocol --offline --locked
    if ($LASTEXITCODE) { throw 'Desktop IPC tests failed' }
    & $taskCargo +stable-x86_64-pc-windows-msvc build --manifest-path apps/desktop/src-tauri/Cargo.toml --features tauri/custom-protocol --offline --locked
    if ($LASTEXITCODE) { throw 'Desktop build failed' }
} finally {
    Pop-Location
    foreach ($taskKey in $taskKeys) { [Environment]::SetEnvironmentVariable($taskKey, $taskPrevious[$taskKey], 'Process') }
}
