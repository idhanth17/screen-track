# Downloads the bundled AI model weights (all-MiniLM-L6-v2) that the app embeds
# into the installer. Run once on a fresh clone before `npx @tauri-apps/cli build`.
# The weights (~87MB) are gitignored; config.json + tokenizer.json are committed.
#
#   powershell -ExecutionPolicy Bypass -File scripts/fetch-model.ps1

$ErrorActionPreference = "Stop"
$dir  = Join-Path $PSScriptRoot "..\app\src-tauri\resources\minilm"
$base = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main"
New-Item -ItemType Directory -Force -Path $dir | Out-Null

foreach ($f in @("config.json", "tokenizer.json", "model.safetensors")) {
  $dest = Join-Path $dir $f
  if (Test-Path $dest) { Write-Host "have  $f"; continue }
  Write-Host "fetch $f ..."
  Invoke-WebRequest -Uri "$base/$f" -OutFile $dest
}
Write-Host "Model ready in $dir"
