# Generates the Tauri updater manifest (latest.json) for the current version,
# using the signature produced by a signed build. Run AFTER a signed
# `npx @tauri-apps/cli build` (TAURI_SIGNING_PRIVATE_KEY set).
#
#   powershell -ExecutionPolicy Bypass -File scripts/make-latest-json.ps1
#
# Then upload BOTH the renamed installer (ScreenTrack-Setup.exe) and latest.json
# as assets on the release tagged v<version>.
$ErrorActionPreference = "Stop"
$root = Join-Path $PSScriptRoot ".."
$conf = Get-Content (Join-Path $root "app\src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$version = $conf.version

$nsis = Join-Path $root "target\release\bundle\nsis"
$setup = Get-ChildItem $nsis -Filter "*_$($version)_x64-setup.exe" | Select-Object -First 1
$sig   = Get-ChildItem $nsis -Filter "*_$($version)_x64-setup.exe.sig" | Select-Object -First 1
if (-not $setup -or -not $sig) { throw "Signed NSIS installer or .sig for v$version not found in $nsis - build with TAURI_SIGNING_PRIVATE_KEY set." }

$signature = (Get-Content $sig.FullName -Raw).Trim()
$url = "https://github.com/idhanth17/screen-track/releases/download/v$version/ScreenTrack-Setup.exe"
$pubDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$json = @"
{
  "version": "$version",
  "notes": "See the release notes on GitHub.",
  "pub_date": "$pubDate",
  "platforms": {
    "windows-x86_64": {
      "signature": "$signature",
      "url": "$url"
    }
  }
}
"@

$out = Join-Path $root "dist\latest.json"
New-Item -ItemType Directory -Force -Path (Split-Path $out) | Out-Null
# Write UTF-8 WITHOUT a BOM — a leading BOM breaks the updater's JSON parser.
[System.IO.File]::WriteAllText($out, $json, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "Wrote $out (version $version)"
Write-Host "Installer: $($setup.FullName)"
