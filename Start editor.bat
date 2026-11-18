@echo off
title PoE Filter Editor
cd /d "%~dp0"
echo Getting the latest version...
git pull --ff-only
call npm install --no-audit --no-fund --loglevel=error
rem Keep checking for new changes every 30 seconds while the editor runs.
start "PoE Filter Editor updates" /min cmd /c "for /l %%i in (0,0,1) do (git pull --ff-only -q >nul 2>&1 & timeout /t 30 /nobreak >nul)"
echo Starting the editor. Leave this window open while you play.
call npm run tauri dev
taskkill /fi "WINDOWTITLE eq PoE Filter Editor updates*" >nul 2>&1
