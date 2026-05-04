# ============================================
#  CinaVault Premium — One-Click Deploy Script
#  Builds installer, copies to desktop, pushes to GitHub
#  Run: Right-click > Run with PowerShell
# ============================================

$ErrorActionPreference = "Stop"
$ProjectDir = $PSScriptRoot
$DesktopBuilds = "$env:USERPROFILE\Desktop\CinaVault Builds"
$GithubRepo = "https://github.com/johngraven75/CinaVault-Premium.git"
$Branch = "tauri-premium"

Write-Host ""
Write-Host "  ╔══════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "  ║  CinaVault Premium — One-Click Deploy        ║" -ForegroundColor Cyan
Write-Host "  ║  Tauri v2 + Rust + React/TypeScript          ║" -ForegroundColor Cyan
Write-Host "  ╚══════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# ---- Check prerequisites ----
Write-Host "[CHECK] Verifying prerequisites..." -ForegroundColor Yellow

$node = Get-Command node -ErrorAction SilentlyContinue
if (-not $node) {
    Write-Host "[ERROR] Node.js not found. Install from https://nodejs.org/" -ForegroundColor Red
    Write-Host "        Recommended: Node.js 20 LTS" -ForegroundColor Red
    pause; exit 1
}
Write-Host "  Node.js: $(node --version)" -ForegroundColor Green

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Write-Host "[ERROR] Rust/Cargo not found. Install from https://rustup.rs/" -ForegroundColor Red
    pause; exit 1
}
Write-Host "  Cargo:   $(cargo --version)" -ForegroundColor Green

$git = Get-Command git -ErrorAction SilentlyContinue
if (-not $git) {
    Write-Host "[WARN] Git not found. GitHub push will be skipped." -ForegroundColor Yellow
} else {
    Write-Host "  Git:     $(git --version)" -ForegroundColor Green
}

# ---- Install dependencies ----
Write-Host ""
Write-Host "[1/5] Installing frontend dependencies..." -ForegroundColor Cyan
Set-Location $ProjectDir
npm install
if ($LASTEXITCODE -ne 0) { Write-Host "[ERROR] npm install failed" -ForegroundColor Red; pause; exit 1 }

# ---- Install Tauri CLI ----
Write-Host ""
Write-Host "[2/5] Ensuring Tauri CLI is installed..." -ForegroundColor Cyan
cargo install tauri-cli --version "^2.0" 2>$null

# ---- Build ----
Write-Host ""
Write-Host "[3/5] Building CinaVault Premium installer..." -ForegroundColor Cyan
Write-Host "       This may take 5-10 minutes on first build..." -ForegroundColor DarkGray
npm run tauri build
if ($LASTEXITCODE -ne 0) { Write-Host "[ERROR] Build failed" -ForegroundColor Red; pause; exit 1 }

# ---- Copy to Desktop ----
Write-Host ""
Write-Host "[4/5] Copying installers to Desktop..." -ForegroundColor Cyan
if (-not (Test-Path $DesktopBuilds)) { New-Item -ItemType Directory -Path $DesktopBuilds -Force | Out-Null }

$nsisFiles = Get-ChildItem "src-tauri\target\release\bundle\nsis\*.exe" -ErrorAction SilentlyContinue
$msiFiles  = Get-ChildItem "src-tauri\target\release\bundle\msi\*.msi" -ErrorAction SilentlyContinue

foreach ($f in $nsisFiles) {
    Copy-Item $f.FullName $DesktopBuilds -Force
    Write-Host "  Copied: $($f.Name)" -ForegroundColor Green
}
foreach ($f in $msiFiles) {
    Copy-Item $f.FullName $DesktopBuilds -Force
    Write-Host "  Copied: $($f.Name)" -ForegroundColor Green
}

Write-Host "  Installers saved to: $DesktopBuilds" -ForegroundColor Green

# ---- Push to GitHub ----
Write-Host ""
Write-Host "[5/5] Pushing to GitHub as Public Beta 1..." -ForegroundColor Cyan

if ($git) {
    Set-Location $ProjectDir

    # Initialize git if needed
    if (-not (Test-Path ".git")) {
        git init
        git remote add origin $GithubRepo
    }

    git checkout -B $Branch
    git add -A
    git commit -m "CinaVault Premium — Tauri v2 Rewrite (Public Beta 1)

Engine: Tauri v2 + Rust + React/TypeScript
Build: v109 Premium Rewrite
Features: 11-tab UI, glassmorphism, particle effects, 30+ metadata providers,
IPTV, Jellyfin/Emby, VPN, AI diagnostics, plugin system, duplicate finder"

    git push -u origin $Branch --force

    if ($LASTEXITCODE -eq 0) {
        Write-Host "  Pushed to: $GithubRepo (branch: $Branch)" -ForegroundColor Green

        # Create GitHub release with installer
        Write-Host "  Creating GitHub release..." -ForegroundColor Cyan
        $tagName = "v1.0.0-beta.1"

        # Try using GitHub CLI if available
        $gh = Get-Command gh -ErrorAction SilentlyContinue
        if ($gh) {
            $installerFiles = @()
            foreach ($f in $nsisFiles) { $installerFiles += $f.FullName }
            foreach ($f in $msiFiles)  { $installerFiles += $f.FullName }

            $releaseArgs = @(
                "release", "create", $tagName,
                "--title", "CinaVault-Premium-v1.0.0 (Public Beta 1)",
                "--notes", "CinaVault Premium - Tauri v2 + Rust + React/TypeScript rewrite. Public Beta 1.",
                "--prerelease"
            )
            foreach ($f in $installerFiles) { $releaseArgs += $f }

            & gh @releaseArgs
            if ($LASTEXITCODE -eq 0) {
                Write-Host "  Release created with installers attached!" -ForegroundColor Green
            }
        } else {
            Write-Host "  [INFO] Install GitHub CLI (gh) to auto-create releases: winget install GitHub.cli" -ForegroundColor Yellow
            Write-Host "  Manual: Go to https://github.com/johngraven75/CinaVault-Premium/releases/new" -ForegroundColor Yellow
        }
    } else {
        Write-Host "  [WARN] Push failed. You may need to authenticate:" -ForegroundColor Yellow
        Write-Host "         git push -u origin $Branch" -ForegroundColor Yellow
    }
} else {
    Write-Host "  [SKIP] Git not installed. Push manually after installing Git." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "  ============================================" -ForegroundColor Green
Write-Host "   DEPLOY COMPLETE!" -ForegroundColor Green
Write-Host "   Installers: $DesktopBuilds" -ForegroundColor Green
Write-Host "   GitHub:     $GithubRepo ($Branch)" -ForegroundColor Green
Write-Host "  ============================================" -ForegroundColor Green
Write-Host ""
pause
