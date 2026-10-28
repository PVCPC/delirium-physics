@echo off
rem Delirium room in Safe Mode: every graphics setting at its lowest and no upscaler hooks (plain Vulkan), your saved settings left alone. Same as delirium-room.bat (keep it beside this file); drag your game's .iso onto this one.
if not exist "%~dp0delirium-room.bat" (echo Put this file in the same folder as delirium-room.bat. & pause & exit /b 1)
set "SAFE=--safe"
call "%~dp0delirium-room.bat" %*
