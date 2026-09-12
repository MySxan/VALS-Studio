# Compatibility entry point; validation is shared with CI.
#Requires -Version 7.0
[CmdletBinding()]
param(
    [ValidateSet('Auto', 'Local', 'System')][string]$Environment = 'Auto',
    [switch]$DiagnoseOnly,
    [switch]$RefreshFixture
)
& "$PSScriptRoot/verify.ps1" -Scope Desktop -Environment $Environment -DiagnoseOnly:$DiagnoseOnly -RefreshFixture:$RefreshFixture
