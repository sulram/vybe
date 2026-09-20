#!/bin/sh
# Renders the PNG sequences map-show.vy plays — with vybe itself, headless.
# Run from anywhere:  examples/patches/map-show/make-media.sh
set -e
cd "$(dirname "$0")"
vybe() { cargo run -q --release -p vybe-cli -- "$@"; }

vybe render media/transicao.vy --seq 4s --fps 30 --size 600x600 --alpha \
     --osc "/go 1 @0s" --out media/transicao > /dev/null
vybe render media/principal.vy --seq 5s --fps 30 --size 600x600 \
     --osc "/go 1 @0s" --out media/principal > /dev/null
echo "media/transicao: $(ls media/transicao | wc -l | tr -d ' ') frames"
echo "media/principal: $(ls media/principal | wc -l | tr -d ' ') frames"
