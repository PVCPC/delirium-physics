@echo off
rem Delirium room. Drag your game's disc image (.iso) onto this file, or pick the game folder when the room asks.
setlocal
cd /d "%~dp0"
rem On its own (downloaded from the release page): fetch this version's ready-built room beside it, no build needed.
set "VERSION=0.2.5"
if exist "%~dp0Cargo.toml" goto build
rem Beside the ready-built room (unzipped together): run it.
set "VIEWER=%~dp0delirium-room.exe"
if exist "%VIEWER%" goto run
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
rem After a fallback from Vulkan (an old graphics driver): Microsoft's DirectX shader compiler, fetched from Microsoft, makes
rem the DirectX 12 mode ready several times faster (not part of Delirium; without it, DirectX 12 just starts slower).
set "DXC=%APPDATA%\delirium\dxc"
if exist "%APPDATA%\delirium\graphics-fallback" if not exist "%DXC%\bin\x64\dxcompiler.dll" (
  echo Downloading Microsoft's DirectX shader compiler, for the DirectX 12 mode...
  mkdir "%DXC%" 2>nul
  curl -sSfL -o "%TEMP%\dxc.zip" "https://github.com/microsoft/DirectXShaderCompiler/releases/download/v1.9.2609/dxc_2026_09_29.zip" && tar -xf "%TEMP%\dxc.zip" -C "%DXC%" bin/x64/dxcompiler.dll
)
rem (delirium-room-safe.bat sets SAFE to --safe: Safe Mode.)
if /i "%~x1"==".iso" ("%VIEWER%" %SAFE% --game "%~f1") else ("%VIEWER%" %SAFE% %*)
set "CODE=%errorlevel%"
rem The graphics driver crashed before the room's first frames (its marker left): again, up to four times; after two
rem crashes in a row the room starts one step safer (plain Vulkan, then DirectX 12).
set /a TRIES+=1
if not "%CODE%"=="0" if %TRIES% LEQ 4 if exist "%APPDATA%\delirium\starting" (echo. & echo The graphics driver crashed starting the room: starting it again... & goto run)
if not "%CODE%"=="0" (echo. & echo The room stopped unexpectedly ^(code %CODE%^): the lines above say why. & pause)
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
