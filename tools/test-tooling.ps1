#Requires -Version 7.0
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/validation.ps1"
function Assert-Tooling($Condition, [string]$Message) { if (!$Condition) { throw $Message } }
function Assert-Throws([scriptblock]$Action, [string]$Pattern) {
    $caught = $null
    try { & $Action | Out-Null } catch { $caught = $_.ToString() }
    Assert-Tooling ($caught -and $caught -match $Pattern) "Expected failure matching: $Pattern; got: $caught"
}
$taskTemp = Join-Path ([IO.Path]::GetTempPath()) ('vals-tooling-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $taskTemp | Out-Null
try {
    # Native nonzero exit must stop the sequence, and environment must restore on exception.
    $taskBefore = $env:VALS_TOOLING_TEST
    $taskMarker = Join-Path $taskTemp 'should-not-exist'
    Assert-Throws {
        Invoke-WithEnvironment @{ VALS_TOOLING_TEST = 'temporary' } {
            Assert-Tooling ($env:VALS_TOOLING_TEST -eq 'temporary') 'Environment was not applied'
            Invoke-Checked (Get-Process -Id $PID).Path @('-NoProfile', '-Command', 'exit 17')
            New-Item -Path $taskMarker -ItemType File | Out-Null
        }
    } 'Command failed \(17\)'
    Assert-Tooling ($env:VALS_TOOLING_TEST -eq $taskBefore) 'Environment leaked after failure'
    Assert-Tooling (!(Test-Path $taskMarker)) 'Checks continued after failure'
    Assert-Throws { Require-File "$taskTemp/missing" 'test dependency' } 'Missing test dependency'
    # Synthetic installations prove version discovery without depending on machine versions.
    $taskVs = "$taskTemp/vs"
    $taskSdk = "$taskTemp/sdk"
    foreach ($version in @('14.9.1', '14.10.1')) {
        foreach ($file in @('bin/Hostx64/x64/link.exe', 'bin/Hostx64/x64/cl.exe', 'lib/x64/libcmt.lib', 'include/vector')) {
            $path = "$taskVs/VC/Tools/MSVC/$version/$file"
            New-Item -ItemType Directory -Path (Split-Path $path) -Force | Out-Null
            New-Item -ItemType File -Path $path | Out-Null
        }
    }
    foreach ($file in @('bin/10.0.9.0/x64/rc.exe', 'Include/10.0.9.0/um/Windows.h',
        'Include/10.0.9.0/ucrt/stdio.h', 'Lib/10.0.9.0/um/x64/kernel32.lib', 'Lib/10.0.9.0/ucrt/x64/ucrt.lib')) {
        $path = "$taskSdk/$file"
        New-Item -ItemType Directory -Path (Split-Path $path) -Force | Out-Null
        New-Item -ItemType File -Path $path | Out-Null
    }
    New-Item -ItemType Directory -Path "$taskSdk/Include/10.0.10.0" | Out-Null
    New-Item -ItemType Directory -Path "$taskVs/VC/Tools/MSVC/14.11.1/bin/Hostx64/x64" -Force | Out-Null
    New-Item -ItemType File -Path "$taskVs/VC/Tools/MSVC/14.11.1/bin/Hostx64/x64/link.exe" | Out-Null
    Invoke-WithEnvironment @{ VALS_TOOLING_TEST = 'outer' } {
        Invoke-WithEnvironment @{ VALS_TOOLING_TEST = 'inner' } { }
        Assert-Tooling ($env:VALS_TOOLING_TEST -eq 'outer') 'Existing variable was not restored after success'
    }
    Assert-Tooling ($env:VALS_TOOLING_TEST -eq $taskBefore) 'Successful nested scopes leaked environment'
    $found = Find-DesktopEnvironment -Root $taskTemp -VsInstall $taskVs -SdkRoot $taskSdk
    Assert-Tooling ($found.LIB -match '14.10.1') 'MSVC versions were not sorted numerically'
    Assert-Tooling ($found.RC -match '10.0.9.0') 'Incomplete SDK should not be selected'
    Assert-Throws { Find-DesktopEnvironment -Root $taskTemp -VsInstall $taskVs -SdkRoot "$taskTemp/absent" } 'No complete x64 Windows SDK'
    Write-Host 'Tooling tests passed: native failure propagation, environment restoration, missing dependencies, version discovery and incomplete SDK rejection.'
} finally {
    # Delete only the uniquely created test directory after validating its resolved location.
    $resolved = [IO.Path]::GetFullPath($taskTemp)
    $parent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
    if ((Split-Path $resolved) -ne $parent -or !(Split-Path $resolved -Leaf).StartsWith('vals-tooling-')) { throw 'Unsafe tooling test cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
