function Resolve-NativeRun([string]$Root, [string]$Directory) {
    $base = [IO.Path]::GetFullPath((Join-Path $Root '.tools/native-acceptance'))
    $path = [IO.Path]::GetFullPath($Directory)
    $id = [guid]::Empty
    if ((Split-Path $path) -ne $base -or ![guid]::TryParse((Split-Path $path -Leaf), [ref]$id)) {
        throw 'Run directory must be an immediate GUID child of .tools/native-acceptance.'
    }
    $item = Get-Item -LiteralPath $path -ErrorAction Stop
    if (!$item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Invalid or redirected run directory.' }
    return $path
}

function Move-NativeFixture([string]$Root, [string]$Directory) {
    $path = Resolve-NativeRun $Root $Directory
    $manifestPath = Join-Path $path 'run.json'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.version -ne 1) { throw 'Unsupported native run manifest.' }
    $source = Join-Path $path 'voice.wav'
    $destination = Join-Path $path 'relocated.wav'
    $original = Join-Path $path 'Untitled.vocalproj'
    if (Test-Path -LiteralPath $destination) { throw 'Relocated fixture already exists; run directories are single-use.' }
    $item = Get-Item -LiteralPath $source -ErrorAction Stop
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing a redirected fixture.' }
    if ((Get-FileHash -LiteralPath $source).Hash -ne $manifest.fixtureHash) { throw 'Fixture changed; refusing to move it.' }
    # Capture the original Save before any mutation. A later inspection checks it unchanged.
    $manifest | Add-Member -NotePropertyName originalProjectHash -NotePropertyValue (Get-FileHash -LiteralPath $original -ErrorAction Stop).Hash -Force
    $manifest | ConvertTo-Json | Set-Content -LiteralPath $manifestPath -Encoding utf8
    # Both paths are explicit files in the validated test directory; no recursive operation.
    Move-Item -LiteralPath $source -Destination $destination -ErrorAction Stop
}
