# Shared implementation for local and CI validation. Requires PowerShell 7.
function Invoke-Checked {
    param([string]$File, [string[]]$Arguments = @())
    $global:LASTEXITCODE = 0
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Command failed ($LASTEXITCODE): $File $($Arguments -join ' ')" }
}

function Require-File([string]$Path, [string]$Description) {
    if (!(Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Missing $Description : $Path" }
    return $Path
}

function Find-DesktopEnvironment {
    param([string]$Root, [string]$VsInstall, [string]$SdkRoot, [string]$SdkLibRoot)
    if (!$VsInstall) {
        $vswhere = Require-File "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" 'Visual Studio discovery tool (vswhere)'
        $VsInstall = @(Invoke-Checked $vswhere @('-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath')) | Select-Object -First 1
        if (!$VsInstall) { throw 'No Visual Studio x64 C++ Build Tools installation found.' }
    }
    $versions = @(Get-ChildItem -LiteralPath "$VsInstall/VC/Tools/MSVC" -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' } | Sort-Object { [version]$_.Name } -Descending)
    $vc = $versions | Where-Object {
        $base = $_.FullName
        @(@('bin/Hostx64/x64/link.exe', 'bin/Hostx64/x64/cl.exe', 'lib/x64/libcmt.lib', 'include/vector') |
            Where-Object { !(Test-Path -LiteralPath "$base/$_" -PathType Leaf) }).Count -eq 0
    } | Select-Object -First 1
    if (!$vc) { throw "No complete x64 MSVC toolset under $VsInstall (link, cl, headers and libraries required)." }
    $vc = $vc.FullName
    if (!$SdkRoot) {
        $isolated = "$Root/.tools/windows-sdk/microsoft.windows.sdk.cpp/c"
        if (Test-Path -LiteralPath $isolated) {
            $SdkRoot = $isolated
            $SdkLibRoot = "$Root/.tools/windows-sdk/microsoft.windows.sdk.cpp.x64/c"
        } else {
            $SdkRoot = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots' -ErrorAction SilentlyContinue).KitsRoot10
            if (!$SdkRoot) { throw 'Windows SDK not found (isolated SDK or Windows Kits registry). Install it explicitly before validation.' }
        }
    }
    $sdkVersions = @(Get-ChildItem -LiteralPath "$SdkRoot/Include" -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } | Sort-Object { [version]$_.Name } -Descending)
    foreach ($version in $sdkVersions) {
        $v = $version.Name
        $lib = if ($SdkLibRoot) { $SdkLibRoot } else { "$SdkRoot/Lib/$v" }
        $required = @("$SdkRoot/bin/$v/x64/rc.exe", "$SdkRoot/Include/$v/um/Windows.h",
            "$SdkRoot/Include/$v/ucrt/stdio.h", "$lib/um/x64/kernel32.lib", "$lib/ucrt/x64/ucrt.lib",
            "$vc/lib/x64/libcmt.lib", "$vc/include/vector")
        if (@($required | Where-Object { !(Test-Path -LiteralPath $_ -PathType Leaf) }).Count -eq 0) {
            return @{
                PATH = "$vc/bin/Hostx64/x64;$SdkRoot/bin/$v/x64;$env:PATH"
                LIB = "$vc/lib/x64;$lib/um/x64;$lib/ucrt/x64"
                INCLUDE = "$vc/include;$SdkRoot/Include/$v/ucrt;$SdkRoot/Include/$v/shared;$SdkRoot/Include/$v/um;$SdkRoot/Include/$v/winrt"
                RC = "$SdkRoot/bin/$v/x64/rc.exe"
            }
        }
    }
    throw "No complete x64 Windows SDK found under $SdkRoot (headers, libraries and rc.exe are required)."
}

function Set-ProcessEnvironment([string]$Key, $Value) {
    if ($null -eq $Value) { [Environment]::SetEnvironmentVariable($Key, [NullString]::Value, 'Process') }
    else { [Environment]::SetEnvironmentVariable($Key, [string]$Value, 'Process') }
}

function Invoke-WithEnvironment {
    param([hashtable]$Values, [scriptblock]$Action)
    $previous = @{}
    try {
        foreach ($key in $Values.Keys) {
            $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
            Set-ProcessEnvironment $key $Values[$key]
        }
        & $Action
    } finally {
        foreach ($key in $previous.Keys) { Set-ProcessEnvironment $key $previous[$key] }
    }
}

function Invoke-CoreChecks {
    param([string]$Cargo, [string[]]$Toolchain)
    Write-Host 'Core: formatting, lint, tests and examples'
    Invoke-Checked $Cargo @('fmt', '--all', '--', '--check')
    Invoke-Checked $Cargo ($Toolchain + @('clippy', '--workspace', '--all-targets', '--offline', '--locked', '--', '-D', 'warnings'))
    Invoke-Checked $Cargo ($Toolchain + @('test', '--workspace', '--offline', '--locked'))
    foreach ($example in @(@('vocal-time', 'tempo_map'), @('vocal-project', 'project_roundtrip'), @('vocal-project', 'import_wav'), @('vocal-analysis-runtime', 'analyze_waveform'), @('vocal-app', 'import_session'))) {
        Invoke-Checked $Cargo ($Toolchain + @('run', '-p', $example[0], '--example', $example[1], '--offline', '--locked')) | Out-Null
        Write-Host "Example passed: $($example[1])"
    }
}

function Invoke-DesktopChecks {
    param([string]$Root, [string]$Cargo, [string[]]$Toolchain, [switch]$RefreshFixture)
    Write-Host 'Desktop: Rust/TypeScript fixture contract, frontend tests/build and native checks'
    $fixture = Invoke-Checked $Cargo ($Toolchain + @('run', '-p', 'vocal-app', '--example', 'import_session', '--offline', '--locked'))
    $fixtureMode = if ($RefreshFixture) { '--refresh' } else { '--check' }
    # Send JSON through stdin; normal verification never rewrites the reviewed fixture.
    $global:LASTEXITCODE = 0
    $fixture | & node "$Root/tools/fixture.mjs" $fixtureMode "$Root/apps/desktop/src/test-fixtures/session.json"
    if ($LASTEXITCODE) { throw 'Rust DTO fixture contract failed. Review the change before using -RefreshFixture.' }
    Invoke-Checked node @('--test', "$Root/tools/fixture.test.mjs")
    Push-Location "$Root/apps/desktop"
    try {
        Invoke-Checked npm.cmd @('test')
        Invoke-Checked npm.cmd @('run', 'build')
    } finally { Pop-Location }
    $manifest = @('--manifest-path', 'apps/desktop/src-tauri/Cargo.toml')
    Invoke-Checked $Cargo (@('fmt') + $manifest + @('--', '--check'))
    Invoke-Checked $Cargo ($Toolchain + @('clippy') + $manifest + @('--all-targets', '--features', 'tauri/custom-protocol', '--offline', '--locked', '--', '-D', 'warnings'))
    foreach ($command in @('test', 'build')) {
        Invoke-Checked $Cargo ($Toolchain + @($command) + $manifest + @('--features', 'tauri/custom-protocol', '--offline', '--locked'))
    }
}
