[CmdletBinding()]
param(
    # File Tauri asks us to sign: the app executable, the NSIS installer and
    # uninstaller, or the MSI. Tauri substitutes it for %1 in signCommand.
    [Parameter(Mandatory = $true, Position = 0)][string]$Path
)

# Signs one file with Azure Artifact Signing (Microsoft's code-signing
# service). Called by `tauri build` through bundle.windows.signCommand, which
# the release workflow only sets after configure-windows-signing.ps1 found the
# signing configuration. See docs/CODE_SIGNING.md.

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
if ($missing.Count -gt 0) {
    throw "Code signing is not configured; missing $($missing -join ', '). See docs/CODE_SIGNING.md."
}
if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "File to sign was not found: $Path"
}

$description = if ($env:CINAVAULT_SIGNING_DESCRIPTION) { $env:CINAVAULT_SIGNING_DESCRIPTION } else { 'CinaVault Premium' }

# Account, certificate profile and the Azure credentials are read from the
# environment by artifact-signing-cli, so no secret appears on a command line.
& artifact-signing-cli -e $env:AZURE_ARTIFACT_SIGNING_ENDPOINT -d $description $Path
if ($LASTEXITCODE -ne 0) {
    throw "Signing failed for $Path (exit code $LASTEXITCODE)."
}
Write-Host "Signed $Path"
