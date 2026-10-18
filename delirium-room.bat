@echo off
rem Delirium room. Drag your game's disc image (.iso) onto this file, or pick the game folder when the room asks.
setlocal
cd /d "%~dp0"
rem On its own (downloaded from the release page): fetch this version's ready-built room beside it, no build needed.
set "VERSION=0.1.6"
if exist "%~dp0Cargo.toml" goto build
set "DIR=%~dp0delirium-room-%VERSION%"
set "VIEWER=%DIR%\delirium-room.exe"
if exist "%VIEWER%" goto run
echo Downloading Delirium Physics %VERSION%...
curl -sSfL -o "%TEMP%\delirium-room-%VERSION%.zip" "https://github.com/PVCPC/delirium-physics/releases/download/v%VERSION%/delirium-room-%VERSION%-windows.zip" || (echo Download failed. & pause & exit /b 1)
mkdir "%DIR%" 2>nul
tar -xf "%TEMP%\delirium-room-%VERSION%.zip" -C "%DIR%" || (echo Unzip failed. & pause & exit /b 1)
goto run

rem In the source (the repo): build it, then run.
:build
set "VIEWER=%~dp0target\release\delirium-room.exe"
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>&1
if errorlevel 1 call :install || exit /b 1
echo Building the room (the first time takes a few minutes)...
cargo build --release -p delirium-room --manifest-path "%~dp0Cargo.toml"
if errorlevel 1 (echo Build failed. & pause & exit /b 1)

:run
if /i "%~x1"==".iso" ("%VIEWER%" --game "%~f1") else ("%VIEWER%" %*)
if not "%errorlevel%"=="0" (echo. & echo The room stopped unexpectedly ^(code %errorlevel%^): the lines above say why. & pause)
exit /b 0

:install
echo The room is built from source: it needs Rust and Microsoft's C++ build tools (free; a few GB).
choice /m "Install them now (winget)"
if errorlevel 2 exit /b 1
winget install -e --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install -e --id Rustlang.Rustup
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>&1
if errorlevel 1 (echo Rust didn't install. Get it from https://rustup.rs, then run this again. & pause & exit /b 1)
exit /b 0
