# CinaVault Premium — Windows Installer Build Script
# Builds the production web app, validates the Rust/Tauri side, and creates Windows installer bundles.

param(
    [switch]$SkipTests,
    [switch]$NoDesktopCopy,
    [switch]$NoOpenDesktop
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $RepoRoot

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Require-Command {
    param(
        [string]$Name,
        [string]$InstallHint
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found. $InstallHint"
    }
}

function Invoke-Checked {
    param(
        [string]$Command,
        [string[]]$Arguments
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code $LASTEXITCODE: $Command $($Arguments -join ' ')"
    }
}

Write-Host "CinaVault Premium Windows Installer Build" -ForegroundColor Magenta
Write-Host "Repository: $RepoRoot"

Write-Step "Checking required tools"
Require-Command "node" "Install Node.js LTS from https://nodejs.org/."
Require-Command "npm" "Install Node.js LTS from https://nodejs.org/."
Require-Command "cargo" "Install Rust from https://rustup.rs/."
Require-Command "rustc" "Install Rust from https://rustup.rs/."

Write-Step "Installing JavaScript dependencies from patched manifest"
Invoke-Checked "npm" @("install")

Write-Step "Running TypeScript build"
Invoke-Checked "npm" @("run", "build")

if (-not $SkipTests) {
    Write-Step "Running Rust compile check"
    Invoke-Checked "cargo" @("check", "--manifest-path", "src-tauri/Cargo.toml")

    Write-Step "Running PGMA bridge tests"
    Invoke-Checked "cargo" @("test", "--manifest-path", "src-tauri/Cargo.toml", "pgma_bridge", "--", "--nocapture")

    Write-Step "Running PGMA plugin deployer tests"
    Invoke-Checked "cargo" @("test", "--manifest-path", "src-tauri/Cargo.toml", "plugins::tests", "--", "--nocapture")
} else {
    Write-Step "Skipping tests because -SkipTests was supplied"
}

Write-Step "Building Windows installer with Tauri"
Invoke-Checked "npm" @("run", "tauri", "--", "build")

Write-Step "Finding installer outputs"
$Installers = @()
$BundleRoot = Join-Path $RepoRoot "src-tauri\target\release\bundle"
if (Test-Path $BundleRoot) {
    $Installers = Get-ChildItem -Path $BundleRoot -Recurse -File | Where-Object {
        $_.Extension -in ".exe", ".msi", ".zip"
    }
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

    if (-not $NoOpenDesktop) {
        Start-Process explorer.exe $OutDir
    }
}

Write-Host ""
Write-Host "Build complete." -ForegroundColor Green
