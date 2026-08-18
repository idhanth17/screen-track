# Screen Track — one-command installer for Windows.
#
# For end users: no Rust, no Node, no build tools, no admin rights. This downloads
# the prebuilt, self-contained installer (the AI model is baked in), installs it
# for the current user, puts an icon on the Desktop, and launches it.
#
#   irm https://raw.githubusercontent.com/idhanth17/screen-track/master/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"
$product = "Screen Track"
$asset   = "https://github.com/idhanth17/screen-track/releases/latest/download/ScreenTrack-Setup.exe"
$tmp     = Join-Path $env:TEMP "ScreenTrack-Setup.exe"

# Smart App Control (Windows 11) hard-blocks unsigned installers with no bypass.
$sac = 0
try { $sac = (Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy" -Name VerifiedAndReputablePolicyState -ErrorAction Stop).VerifiedAndReputablePolicyState } catch {}

Write-Host "Downloading $product ..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $asset -OutFile $tmp
try { Unblock-File $tmp } catch {}

if ($sac -eq 1) {
  Write-Host ""
  Write-Warning "Smart App Control is ON, which blocks apps that aren't code-signed (this one isn't)."
  Write-Host "To install Screen Track, turn Smart App Control off first:" -ForegroundColor Yellow
  Write-Host "  Start > 'Smart App Control' (or Windows Security > App & browser control > Smart App Control settings) > Off"
  Write-Host "  (Windows only lets you re-enable it by resetting the PC, so this is a real choice.)"
  Write-Host ""
  Write-Host "Then re-run this command. The installer is already downloaded here if you prefer to run it yourself:"
  Write-Host "  $tmp"
  return
}

Write-Host "Installing (no admin needed) ..." -ForegroundColor Cyan
try {
  Start-Process -FilePath $tmp -ArgumentList "/S" -Wait
} catch {
  Write-Host ""
  Write-Warning "Windows blocked the installer: $($_.Exception.Message)"
  Write-Host "This is usually Smart App Control. Turn it off (Windows Security > App & browser control >"
  Write-Host "Smart App Control settings > Off), then re-run this command."
  return
}
Start-Sleep -Seconds 2

# Locate the installed executable (Tauri installs per-user under LOCALAPPDATA).
$exe = Join-Path $env:LOCALAPPDATA "$product\$product.exe"
if (-not (Test-Path $exe)) {
  $found = Get-ChildItem -Path $env:LOCALAPPDATA -Filter "$product.exe" -Recurse -ErrorAction SilentlyContinue |
           Select-Object -First 1
  if ($found) { $exe = $found.FullName }
}

if (Test-Path $exe) {
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
  Write-Host "It starts automatically at login. The AI classifier is built in and already active." -ForegroundColor Green
} else {
  Write-Warning "Installed, but couldn't auto-launch. Open '$product' from the Start Menu."
}
