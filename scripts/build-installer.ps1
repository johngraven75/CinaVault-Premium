# CinaVault Premium Windows Installer Build Script
# Build 140 cleanup: installer validation now uses the Build 140 regression suite.

param(
    [switch]$SkipTests,
    [switch]$NoDesktopCopy,
    [switch]$NoOpenDesktop
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

Set-Location $PSScriptRoot\..

function Invoke-Step {
    param(
        [string]$Label,
        [scriptblock]$Action
    )

    Write-Host ""
    Write-Host "==> $Label" -ForegroundColor Cyan
    & $Action
    if ($LASTEXITCODE -ne 0) {
        throw "Step failed: $Label"
    }
}

Invoke-Step "Installing JavaScript dependencies" { npm install --legacy-peer-deps --loglevel verbose }
Invoke-Step "Running TypeScript simulation build" { npx tsc -p tsconfig.build.json }
Invoke-Step "Running Vite production build" { npx vite build }

if (-not $SkipTests) {
    Invoke-Step "Running Build 140 JavaScript surface regression tests" { npm run test:build140 }
    Invoke-Step "Running Rust compile check" { cargo check --manifest-path src-tauri/Cargo.toml }
    Invoke-Step "Running scanner ingestion regression tests" { cargo test --manifest-path src-tauri/Cargo.toml scanner::tests -- --nocapture }
    Invoke-Step "Running metadata poster posting regression test" { cargo test --manifest-path src-tauri/Cargo.toml metadata_posting_tests -- --nocapture }
    Invoke-Step "Running PGMA bridge tests" { cargo test --manifest-path src-tauri/Cargo.toml pgma_bridge -- --nocapture }
    Invoke-Step "Running PGMA plugin deployer tests" { cargo test --manifest-path src-tauri/Cargo.toml plugins::tests -- --nocapture }
}

$env:RUST_BACKTRACE = "full"
$env:TAURI_DEBUG = "true"
Invoke-Step "Building Windows installer with Tauri" { npx tauri build }

$BundleRoot = Join-Path $PWD "src-tauri\target\release\bundle"
$Installers = @()
if (Test-Path $BundleRoot) {
    $Installers = Get-ChildItem -Path $BundleRoot -Recurse -File | Where-Object { $_.Extension -in ".exe", ".msi", ".zip" }
}

if (-not $Installers -or $Installers.Count -eq 0) {
    throw "No installer artifacts were produced under $BundleRoot."
}

Write-Host "Installer artifacts:" -ForegroundColor Green
foreach ($Installer in $Installers) {
    Write-Host " - $($Installer.FullName)"
}

if (-not $NoDesktopCopy) {
    $Desktop = [Environment]::GetFolderPath("Desktop")
    $OutDir = Join-Path $Desktop "CinaVault-Premium-Installer"
    New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
    foreach ($Installer in $Installers) {
        Copy-Item -Path $Installer.FullName -Destination $OutDir -Force
    }
    Write-Host "Copied installers to: $OutDir" -ForegroundColor Green
}

Write-Host ""
Write-Host "Build complete." -ForegroundColor Green
