#!/bin/sh
# Renders the PNG sequence map-show.vy scrubs — with vybe itself, headless.
# Run from anywhere:  examples/patches/map-show/make-media.sh
set -e
cd "$(dirname "$0")"
vybe() { cargo run -q --release -p vybe-cli -- "$@"; }

vybe render media/transicao.vy --seq 4s --fps 30 --size 600x600 --alpha \
     --osc "/go 1 @0s" --out media/transicao > /dev/null
echo "media/transicao: $(ls media/transicao | wc -l | tr -d ' ') frames"
