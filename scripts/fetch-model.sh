#!/usr/bin/env bash
# Downloads the bundled AI model weights (all-MiniLM-L6-v2) that the app embeds
# into the installer. Run once on a fresh clone before building.
# The weights (~87MB) are gitignored; config.json + tokenizer.json are committed.
#
#   bash scripts/fetch-model.sh
set -euo pipefail

dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/app/src-tauri/resources/minilm"
base="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main"
mkdir -p "$dir"

for f in config.json tokenizer.json model.safetensors; do
  if [ -f "$dir/$f" ]; then echo "have  $f"; continue; fi
  echo "fetch $f ..."
  curl -fSL -o "$dir/$f" "$base/$f"
done
echo "Model ready in $dir"
