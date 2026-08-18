# Packages the browser extension into scripts/../dist/screen-track-extension.zip
# for attaching to a GitHub release (users download, unzip, Load unpacked).
$ErrorActionPreference = "Stop"
$root = Join-Path $PSScriptRoot ".."
$src  = Join-Path $root "extension"
$out  = Join-Path $root "dist"
$stage = Join-Path $out "screen-track-extension"
$zip  = Join-Path $out "screen-track-extension.zip"

New-Item -ItemType Directory -Force -Path $out | Out-Null
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
Copy-Item -Recurse $src $stage
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Remove-Item -Recurse -Force $stage
Write-Host "Wrote $zip"
