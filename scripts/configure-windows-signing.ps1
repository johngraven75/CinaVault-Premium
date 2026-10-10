[CmdletBinding()]
param(
    # Where to write the Tauri config overlay that turns signing on.
    [string]$ConfigPath = (Join-Path ([IO.Path]::GetTempPath()) 'cinavault-signing.tauri.json'),
    # Absolute path of sign-windows.ps1; defaults to the copy next to this script.
    [string]$SignScript = (Join-Path $PSScriptRoot 'sign-windows.ps1')
)

# Decides whether the release build signs its binaries. With every Azure
# Artifact Signing setting present it writes a Tauri config overlay whose
# bundle.windows.signCommand runs sign-windows.ps1 on each file, and exports
# CINAVAULT_SIGNING_CONFIG for the build step. Otherwise the build stays
# unsigned and a warning says what is missing. See docs/CODE_SIGNING.md.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$required = @(
    'AZURE_TENANT_ID',
    'AZURE_CLIENT_ID',
    'AZURE_CLIENT_SECRET',
    'AZURE_ARTIFACT_SIGNING_ENDPOINT',
    'AZURE_ARTIFACT_SIGNING_ACCOUNT',
    'AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE'
)
$missing = @($required | Where-Object { [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($_)) })

function Write-ActionEnv([string]$Name, [string]$Value) {
    if ($env:GITHUB_ENV) { Add-Content -LiteralPath $env:GITHUB_ENV -Value "$Name=$Value" -Encoding utf8 }
}
function Write-ActionOutput([string]$Name, [string]$Value) {
    if ($env:GITHUB_OUTPUT) { Add-Content -LiteralPath $env:GITHUB_OUTPUT -Value "$Name=$Value" -Encoding utf8 }
}

if ($missing.Count -gt 0) {
    Write-Host "::warning title=Installers will be unsigned::Windows will show an unknown-publisher warning. Missing: $($missing -join ', '). See docs/CODE_SIGNING.md."
    Write-ActionOutput 'enabled' 'false'
    return
}

$signScript = (Resolve-Path -LiteralPath $SignScript).Path
$overlay = @{
    bundle = @{
        windows = @{
            signCommand = @{
                cmd  = 'pwsh'
                args = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', $signScript, '%1')
            }
        }
    }
}
$overlay | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ConfigPath -Encoding utf8
Write-ActionEnv 'CINAVAULT_SIGNING_CONFIG' $ConfigPath
Write-ActionOutput 'enabled' 'true'
Write-Host "Code signing enabled with Artifact Signing account $env:AZURE_ARTIFACT_SIGNING_ACCOUNT."
