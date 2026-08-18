# Screen Track — one-command installer for Windows.
#
# For end users: no Rust, no Node, no build tools, no admin rights. This just
# downloads the prebuilt, self-contained installer (the AI model is baked in),
# installs it for the current user, puts an icon on the Desktop, and launches it.
#
# Run it with a single line in PowerShell:
#   irm https://raw.githubusercontent.com/idhanth17/screen-track/master/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"
$product = "Screen Track"
$asset   = "https://github.com/idhanth17/screen-track/releases/latest/download/ScreenTrack-Setup.exe"
$tmp     = Join-Path $env:TEMP "ScreenTrack-Setup.exe"

Write-Host "Downloading $product ..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $asset -OutFile $tmp

Write-Host "Installing (no admin needed) ..." -ForegroundColor Cyan
# Tauri's NSIS installer supports a silent per-user install with /S.
Start-Process -FilePath $tmp -ArgumentList "/S" -Wait
Start-Sleep -Seconds 2

# Locate the installed executable (Tauri installs per-user under LOCALAPPDATA).
$exe = Join-Path $env:LOCALAPPDATA "$product\$product.exe"
if (-not (Test-Path $exe)) {
  $found = Get-ChildItem -Path $env:LOCALAPPDATA -Filter "$product.exe" -Recurse -ErrorAction SilentlyContinue |
           Select-Object -First 1
  if ($found) { $exe = $found.FullName }
}

if (Test-Path $exe) {
  # Desktop icon so it's easy to find.
  try {
    $desktop = [Environment]::GetFolderPath("Desktop")
    $ws = New-Object -ComObject WScript.Shell
    $sc = $ws.CreateShortcut((Join-Path $desktop "$product.lnk"))
    $sc.TargetPath = $exe
    $sc.Save()
  } catch { }

  Write-Host "Launching $product ..." -ForegroundColor Cyan
  Start-Process $exe
  Write-Host ""
  Write-Host "$product is installed and running in your system tray." -ForegroundColor Green
  Write-Host "It will start automatically every time you log in. The AI classifier is built in and already active." -ForegroundColor Green
} else {
  Write-Warning "Installed, but couldn't auto-launch. Open '$product' from the Start Menu."
}
