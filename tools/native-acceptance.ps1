#Requires -Version 7.0
[CmdletBinding()]
param(
    [ValidateSet('Prepare', 'Relocate', 'Inspect')][string]$Mode = 'Prepare',
    [string]$RunDirectory
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/validation.ps1"
. "$PSScriptRoot/native-acceptance-lib.ps1"
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskExe = Require-File "$taskRoot/apps/desktop/src-tauri/target/debug/vals-desktop.exe" 'built desktop artifact'
if ($Mode -eq 'Prepare') {
    if ($RunDirectory) { throw 'Prepare creates its own unique run directory.' }
    $taskPath = Join-Path $taskRoot ('.tools/native-acceptance/' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $taskPath | Out-Null
    Copy-Item -LiteralPath "$taskRoot/fixtures/audio/stereo-48000.wav" -Destination "$taskPath/voice.wav"
    @{
        version = 1
        createdAt = [DateTime]::UtcNow.ToString('o')
        executableHash = (Get-FileHash -LiteralPath $taskExe).Hash
        fixtureHash = (Get-FileHash -LiteralPath "$taskPath/voice.wav").Hash
        uiStatus = 'not-observed'
    } | ConvertTo-Json | Set-Content -LiteralPath "$taskPath/run.json" -Encoding utf8
    Write-Output $taskPath
    Write-Host 'Ready for native UI exercise. No app was launched and no UI result is asserted.'
    return
}
if (!$RunDirectory) { throw '-RunDirectory is required.' }
$taskPath = Resolve-NativeRun $taskRoot $RunDirectory
if ($Mode -eq 'Relocate') {
    Move-NativeFixture $taskRoot $taskPath
    Write-Host 'Fixture moved. Verify offline in the app, Relink relocated.wav, then Save As Relinked.vocalproj.'
    return
}
$taskManifest = Get-Content -LiteralPath "$taskPath/run.json" -Raw | ConvertFrom-Json
if ($taskManifest.version -ne 1) { throw 'Unsupported native run manifest.' }
if ((Get-FileHash -LiteralPath $taskExe).Hash -ne $taskManifest.executableHash) { throw 'Build artifact changed since preparation; start a new UI run.' }
if (!$taskManifest.originalProjectHash -or (Get-FileHash -LiteralPath "$taskPath/Untitled.vocalproj").Hash -ne $taskManifest.originalProjectHash) { throw 'Original project changed or Relocate checkpoint is missing.' }
$taskLocal = Test-Path "$taskRoot/.tools/cargo/bin/cargo.exe"
$taskEnv = @{ RUSTUP_AUTO_INSTALL = '0' }
if ($taskLocal) {
    $taskEnv.CARGO_HOME = "$taskRoot/.tools/cargo"
    $taskEnv.RUSTUP_HOME = "$taskRoot/.tools/rustup"
    $taskDlltool = Require-File "$taskRoot/.tools/rustup/toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/llvm-dlltool.exe" 'local GNU dlltool'
    $taskEnv.CARGO_ENCODED_RUSTFLAGS = '-C' + [char]31 + 'dlltool=' + $taskDlltool
}
Push-Location $taskRoot
try {
    Invoke-WithEnvironment $taskEnv {
        $cargo = if ($taskLocal) { "$taskRoot/.tools/cargo/bin/cargo.exe" } else { (Get-Command cargo).Source }
        $chain = @(if ($taskLocal) { '+stable-x86_64-pc-windows-gnu' })
        $result = Invoke-Checked $cargo ($chain + @('run', '-p', 'vocal-app', '--example', 'verify_native_acceptance', '--offline', '--locked', '--', $taskPath))
        $result | ConvertFrom-Json | Out-Null
        $result | Set-Content -LiteralPath "$taskPath/filesystem-report.json" -Encoding utf8
    }
} finally { Pop-Location }
Write-Host 'File/application checks passed. Native window observations must be recorded separately; they are not inferred from these checks.'
