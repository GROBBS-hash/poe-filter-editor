@echo off
cd /d "%~dp0"
powershell -NoProfile -Command "$s=(New-Object -ComObject WScript.Shell).CreateShortcut([Environment]::GetFolderPath(Desktop)+\PoE Filter Editor.lnk); $s.TargetPath=%~dp0Start editor.bat; $s.WorkingDirectory=%~dp0; $s.IconLocation=%~dp0src-tauri\icons\icon.ico; $s.Save()"
echo Shortcut "PoE Filter Editor" added to your desktop.
