# Builds the Windows installer, signs it for the updater and publishes it as a GitHub release.
# The installed editor finds it within a few hours (latest.json) and offers "Restart to update".
#
#   pwsh tools/release.ps1 -Notes "Find in stash; rare prices"          # next patch version (0.2.0 -> 0.2.1)
#   pwsh tools/release.ps1 -Version 0.3.0 -Notes "..."                    # a set version
#
# Needs: the signing key at %USERPROFILE%\.tauri\poe-filter-editor.key (never commit it; without it the installed
# editors can't take updates), gh logged in, a clean main branch.
param([string]$Version = "", [string]$Notes = "")
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$key = Join-Path $env:USERPROFILE ".tauri\poe-filter-editor.key"
if (-not (Test-Path $key)) { throw "Signing key missing: $key" }
if (git status --porcelain --untracked-files=no) { throw "Commit your changes first" }

# Version: tauri.conf.json is what the app and updater read; Cargo.toml follows it.
$confPath = "src-tauri\tauri.conf.json"
$conf = Get-Content $confPath -Raw | ConvertFrom-Json
if (-not $Version) {
  $p = $conf.version.Split(".")
  $Version = "$($p[0]).$($p[1]).$([int]$p[2] + 1)"
}
(Get-Content $confPath -Raw) -replace '"version": "[^"]+"', "`"version`": `"$Version`"" | Set-Content $confPath -NoNewline
(Get-Content "src-tauri\Cargo.toml" -Raw) -replace '(?m)^version = "[^"]+"', "version = `"$Version`"" | Set-Content "src-tauri\Cargo.toml" -NoNewline
Write-Host "Building $Version..."

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run tauri build
if ($LASTEXITCODE -ne 0) { throw "Build failed" }

$nsis = "target\release\bundle\nsis"
$exe = Get-ChildItem $nsis -Filter "*_${Version}_x64-setup.exe" | Select-Object -First 1
if (-not $exe) { throw "No installer for $Version in $nsis" }
$sig = Get-Content "$($exe.FullName).sig" -Raw
# GitHub turns spaces in file names into dots: give it a name without spaces.
$out = Join-Path $root "target\release-upload"
New-Item -ItemType Directory -Force $out | Out-Null
Get-ChildItem $out | Remove-Item -Force
$name = "PoE-Filter-Editor_${Version}_x64-setup.exe"
Copy-Item $exe.FullName (Join-Path $out $name)
$latest = [ordered]@{
  version = $Version
  notes = $Notes
  pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = @{ "windows-x86_64" = @{ signature = $sig.Trim(); url = "https://github.com/GROBBS-hash/poe-filter-editor/releases/download/v$Version/$name" } }
}
$latest | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $out "latest.json")

git add $confPath src-tauri/Cargo.toml Cargo.lock
git commit -m "Release $Version" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git push
$body = if ($Notes) { $Notes } else { "PoE Filter Editor $Version" }
gh release create "v$Version" (Join-Path $out $name) (Join-Path $out "latest.json") --title "PoE Filter Editor $Version" --notes "$body`n`nDownload $name and run it to install. Installed editors offer this update by themselves."
if ($LASTEXITCODE -ne 0) { throw "Publishing the release failed" }
Write-Host "Released $Version"
