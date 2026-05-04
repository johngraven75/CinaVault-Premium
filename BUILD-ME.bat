@echo off
setlocal EnableDelayedExpansion
title CinaVault Premium - Automated Build
color 0B

echo.
echo  ========================================================
echo   CinaVault Premium - Automated Build (v109 Public Beta)
echo   This script installs ALL prerequisites and builds
echo   the full Windows installer for you.
echo  ========================================================
echo.

REM ---- Check Admin ----
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [INFO] Requesting administrator privileges...
    powershell -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

cd /d "%~dp0"
echo [OK] Running from: %CD%
echo.

REM ---- Check winget ----
where winget >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] winget not found. Please install App Installer from the Microsoft Store.
    echo         https://aka.ms/getwinget
    pause
    exit /b 1
)
echo [OK] winget found

REM ===============================================
REM  STEP 1: Install Node.js if missing
REM ===============================================
echo.
echo [1/6] Checking Node.js...
where node >nul 2>&1
if %errorlevel% neq 0 (
    echo [INSTALL] Node.js not found. Installing via winget...
    winget install OpenJS.NodeJS.LTS --accept-package-agreements --accept-source-agreements
    if !errorlevel! neq 0 (
        echo [ERROR] Node.js installation failed.
        echo         Please install manually from https://nodejs.org/
        pause
        exit /b 1
    )
    echo [OK] Node.js installed. Refreshing PATH...
    set "PATH=%PATH%;C:\Program Files\nodejs;%APPDATA%\npm"
    refreshenv >nul 2>&1
) else (
    for /f "tokens=*" %%v in ('node --version') do echo [OK] Node.js %%v found
)

REM ===============================================
REM  STEP 2: Install Rust if missing
REM ===============================================
echo.
echo [2/6] Checking Rust...
where rustc >nul 2>&1
if %errorlevel% neq 0 (
    echo [INSTALL] Rust not found. Installing via winget...
    winget install Rustlang.Rustup --accept-package-agreements --accept-source-agreements
    if !errorlevel! neq 0 (
        echo [ERROR] Rust installation failed.
        echo         Please install manually from https://rustup.rs/
        pause
        exit /b 1
    )
    echo [OK] Rust installed. Refreshing PATH...
    set "PATH=%PATH%;%USERPROFILE%\.cargo\bin"
    rustup default stable >nul 2>&1
) else (
    for /f "tokens=*" %%v in ('rustc --version') do echo [OK] %%v found
)

REM  Double-check both tools are now accessible
echo.
echo [VERIFY] Final prerequisite check...
where node >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Node.js still not in PATH after install.
    echo         Please CLOSE this window, RESTART your computer, then run this script again.
    pause
    exit /b 1
)
where cargo >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Cargo still not in PATH after install.
    echo         Please CLOSE this window, RESTART your computer, then run this script again.
    pause
    exit /b 1
)
echo [OK] All prerequisites verified!

REM ===============================================
REM  STEP 3: Install frontend dependencies
REM ===============================================
echo.
echo [3/6] Installing frontend dependencies (npm install)...
call npm install
if %errorlevel% neq 0 (
    echo [RETRY] npm install failed. Clearing cache and retrying...
    call npm cache clean --force
    rd /s /q node_modules 2>nul
    del package-lock.json 2>nul
    call npm install
    if !errorlevel! neq 0 (
        echo [ERROR] npm install failed after retry.
        echo         Check your internet connection and try again.
        pause
        exit /b 1
    )
)
echo [OK] Frontend dependencies installed

REM ===============================================
REM  STEP 4: Install Tauri CLI
REM ===============================================
echo.
echo [4/6] Installing Tauri CLI (this may take a few minutes first time)...
call cargo install tauri-cli --version "^2.0" 2>nul
echo [OK] Tauri CLI ready

REM ===============================================
REM  STEP 5: Build the full app + installer
REM ===============================================
echo.
echo [5/6] Building CinaVault Premium...
echo       First build takes 5-15 minutes. Subsequent builds are faster.
echo       Please be patient...
echo.
call npx tauri build
if %errorlevel% neq 0 (
    echo.
    echo [ERROR] Build failed. Common fixes:
    echo   1. Make sure you have Visual Studio Build Tools installed:
    echo      winget install Microsoft.VisualStudio.2022.BuildTools
    echo      Then in Visual Studio Installer, add "Desktop development with C++"
    echo   2. Restart your computer and run this script again
    echo   3. Check the error messages above for specific issues
    echo.
    pause
    exit /b 1
)

REM ===============================================
REM  STEP 6: Copy installers to Desktop
REM ===============================================
echo.
echo [6/6] Copying installers to Desktop...
set "BUILDS=%USERPROFILE%\Desktop\CinaVault Builds"
if not exist "%BUILDS%" mkdir "%BUILDS%"

set FOUND=0
for %%f in (src-tauri\target\release\bundle\nsis\*.exe) do (
    copy /y "%%f" "%BUILDS%\" >nul
    echo   [INSTALLER] %%~nxf
    set FOUND=1
)
for %%f in (src-tauri\target\release\bundle\msi\*.msi) do (
    copy /y "%%f" "%BUILDS%\" >nul
    echo   [INSTALLER] %%~nxf
    set FOUND=1
)

if %FOUND%==0 (
    echo [WARN] No installer files found in expected location.
    echo        Check src-tauri\target\release\bundle\ manually.
) else (
    echo.
    echo  ========================================================
    echo   BUILD COMPLETE!
    echo   Your installers are in: %BUILDS%
    echo   Double-click the .exe or .msi to install CinaVault.
    echo  ========================================================
    explorer "%BUILDS%"
)

echo.
pause
