#Requires -Version 7.0
[CmdletBinding()]
param(
    [ValidateSet('All', 'Core', 'Desktop')][string]$Scope = 'All',
    [ValidateSet('Auto', 'Local', 'System')][string]$Environment = 'Auto',
    [switch]$DiagnoseOnly,
    [switch]$RefreshFixture
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/validation.ps1"
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskDesktop = $Scope -ne 'Core'
if ($taskDesktop -and !$IsWindows) { throw 'Desktop validation currently targets Windows x64. Use -Scope Core on this host.' }
if ($RefreshFixture -and (!$taskDesktop -or $DiagnoseOnly)) { throw '-RefreshFixture requires a Desktop or All validation run.' }
$taskLocal = $Environment -eq 'Local' -or ($Environment -eq 'Auto' -and (Test-Path "$taskRoot/.tools/cargo/bin/cargo.exe"))
$taskEnv = @{ RUSTUP_AUTO_INSTALL = '0' }
if ($taskLocal) {
    $taskEnv.CARGO_HOME = "$taskRoot/.tools/cargo"
    $taskEnv.RUSTUP_HOME = "$taskRoot/.tools/rustup"
}
Push-Location $taskRoot
try {
    Invoke-WithEnvironment $taskEnv {
        $taskCargo = if ($taskLocal) { Require-File "$taskRoot/.tools/cargo/bin/cargo.exe" 'local Cargo' } else { (Get-Command cargo -ErrorAction Stop).Source }
        $taskRustup = if ($taskLocal) { Require-File "$taskRoot/.tools/cargo/bin/rustup.exe" 'local rustup' } else { (Get-Command rustup -ErrorAction Stop).Source }
        $taskCoreChain = @(if ($taskLocal) { '+stable-x86_64-pc-windows-gnu' })
        $taskDesktopChain = @(if ($taskLocal) { '+stable-x86_64-pc-windows-msvc' })
        $taskChains = @('')
        if ($taskLocal) {
            if ($Scope -ne 'Desktop') { $taskChains += 'stable-x86_64-pc-windows-gnu' }
            if ($taskDesktop) { $taskChains += 'stable-x86_64-pc-windows-msvc' }
        }
        foreach ($chain in $taskChains) {
            $which = @(if ($chain) { 'which'; '--toolchain'; $chain } else { 'which' })
            $compiler = Invoke-Checked $taskRustup ($which + @('rustc'))
            $versionInfo = @(Invoke-Checked $compiler @('-vV'))
            Write-Host ($versionInfo -join "`n")
            if (!$chain) { $taskDefaultHost = ($versionInfo | Where-Object { $_ -like 'host:*' }) }
            $component = if (!$chain) { 'rustfmt' } else { 'cargo-clippy' }
            Invoke-Checked $taskRustup ($which + @($component)) | Out-Null
        }
        if (!$taskLocal) { Invoke-Checked $taskRustup @('which', 'cargo-clippy') | Out-Null }
        Invoke-Checked $taskCargo @('--version')
        $taskCoreEnv = @{}
        if ($taskLocal -and $Scope -ne 'Desktop') {
            $dlltool = Require-File "$env:RUSTUP_HOME/toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/llvm-dlltool.exe" 'GNU llvm-dlltool (local toolchain setup)'
            $taskCoreEnv.CARGO_ENCODED_RUSTFLAGS = '-C' + [char]31 + 'dlltool=' + $dlltool
        }
        $taskNativeEnv = @{}
        if ($IsWindows -and ($taskDesktop -or (!$taskLocal -and $taskDefaultHost -like '*-windows-msvc'))) {
            $taskNativeEnv = Find-DesktopEnvironment -Root $taskRoot
            $taskNativeEnv.CARGO_ENCODED_RUSTFLAGS = $null
            if (!$taskLocal) { $taskCoreEnv = $taskNativeEnv }
            Write-Host "Windows resource compiler: $($taskNativeEnv.RC)"
        }
        if ($taskDesktop) {
            $node = Invoke-Checked node @('--version')
            if ([version]$node.TrimStart('v') -lt [version]'22.14.0') { throw "Node >= 22.14.0 required; found $node" }
            $npmVersion = Invoke-Checked npm.cmd @('--version')
            Write-Host "Node $node; npm $npmVersion"
            foreach ($binary in @('vite.cmd', 'vitest.cmd', 'tsc.cmd')) {
                Require-File "$taskRoot/apps/desktop/node_modules/.bin/$binary" 'desktop dependencies (run npm ci explicitly)' | Out-Null
            }
        }
        Write-Host "Preflight passed: scope=$Scope environment=$(if ($taskLocal) {'Local'} else {'System'})"
        if (!$DiagnoseOnly) {
            & "$PSScriptRoot/test-tooling.ps1"
            if ($Scope -ne 'Desktop') { Invoke-WithEnvironment $taskCoreEnv { Invoke-CoreChecks $taskCargo $taskCoreChain } }
            if ($taskDesktop) { Invoke-WithEnvironment $taskNativeEnv { Invoke-DesktopChecks $taskRoot $taskCargo $taskDesktopChain -RefreshFixture:$RefreshFixture } }
            Write-Host 'VALIDATION PASSED'
            if ($taskDesktop) {
                Write-Host 'Artifact: apps/desktop/src-tauri/target/debug/vals-desktop.exe'
                Write-Host 'Not verified here: native dialogs/window close, other operating systems, remote CI execution.'
            }
        }
    }
} finally { Pop-Location }
