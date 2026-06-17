param(
    [switch]$SkipTests,
    [switch]$NoDesktopCopy,
    [switch]$NoOpenDesktop
)

$ErrorActionPreference = "Stop"

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
        throw "Missing required tool '$Name'. $InstallHint"
    }
}

function Get-RepoRoot {
    if ($PSScriptRoot) {
        return $PSScriptRoot
    }
    return (Get-Location).Path
}

$RepoRoot = Get-RepoRoot
Set-Location $RepoRoot

$BuildStamp = Get-Date -Format "yyyyMMdd-HHmmss"
$Desktop = [Environment]::GetFolderPath("Desktop")
$DesktopBuildDir = Join-Path $Desktop "CinaVault-Premium-Build-$BuildStamp"
$BundleDir = Join-Path $RepoRoot "src-tauri\target\release\bundle"
$NotesPath = Join-Path $RepoRoot "BUILD_NOTES_PGMA.md"

Write-Host "CinaVault Premium one-step installer build" -ForegroundColor Green
Write-Host "Repo: $RepoRoot"

Write-Step "Checking required tools"
Require-Command "node" "Install Node.js LTS from https://nodejs.org/."
Require-Command "npm" "Install Node.js LTS from https://nodejs.org/."
Require-Command "cargo" "Install Rust from https://rustup.rs/."
Require-Command "rustc" "Install Rust from https://rustup.rs/."

Write-Step "Installing JavaScript dependencies"
if (Test-Path (Join-Path $RepoRoot "package-lock.json")) {
    npm ci
} else {
    npm install
}

Write-Step "Running TypeScript build"
npm run build

if (-not $SkipTests) {
    Write-Step "Running Rust compile check"
    cargo check --manifest-path src-tauri/Cargo.toml

    Write-Step "Running PGMA bridge tests"
    cargo test --manifest-path src-tauri/Cargo.toml pgma_bridge -- --nocapture

    Write-Step "Running PGMA plugin deployer tests"
    cargo test --manifest-path src-tauri/Cargo.toml plugins::tests -- --nocapture
} else {
    Write-Step "Skipping tests because -SkipTests was supplied"
}

Write-Step "Building Windows installer with Tauri"
npm run tauri -- build

Write-Step "Finding installer outputs"
$Installers = @()
if (Test-Path $BundleDir) {
    $Installers = Get-ChildItem -Path $BundleDir -Recurse -File |
        Where-Object { $_.Extension -in ".exe", ".msi", ".zip" } |
        Sort-Object LastWriteTime -Descending
}

if (-not $Installers -or $Installers.Count -eq 0) {
    throw "Build completed, but no installer files were found under $BundleDir. Check the Tauri build output above."
}

if (-not $NoDesktopCopy) {
    Write-Step "Copying installer and build notes to Desktop"
    New-Item -ItemType Directory -Path $DesktopBuildDir -Force | Out-Null
    foreach ($Installer in $Installers) {
        Copy-Item -Path $Installer.FullName -Destination $DesktopBuildDir -Force
    }
    if (Test-Path $NotesPath) {
        Copy-Item -Path $NotesPath -Destination $DesktopBuildDir -Force
    }

    $SummaryPath = Join-Path $DesktopBuildDir "BUILD_OUTPUT.txt"
    @(
        "CinaVault Premium build output",
        "Built: $(Get-Date -Format o)",
        "Repo: $RepoRoot",
        "Branch: $(git branch --show-current 2>$null)",
        "",
        "Installer files:",
        $Installers | ForEach-Object { "- $($_.Name)" },
        "",
        "Run command used:",
        "powershell -ExecutionPolicy Bypass -File .\build-installer.ps1"
    ) | Set-Content -Path $SummaryPath -Encoding UTF8

    Write-Host ""
    Write-Host "Installer files copied to:" -ForegroundColor Green
    Write-Host $DesktopBuildDir -ForegroundColor Yellow

    if (-not $NoOpenDesktop) {
        Start-Process explorer.exe $DesktopBuildDir
    }
} else {
    Write-Host ""
    Write-Host "Installer files generated under:" -ForegroundColor Green
    Write-Host $BundleDir -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Done. CinaVault Premium installer build completed." -ForegroundColor Green
