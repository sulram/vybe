#!/bin/sh
# Two windows on one machine: a face (a patch, played) and the keystone remote
# that calibrates it — the laptop stand-in for "a Pi on the wall + a MacBook".
#
#   scripts/face-and-remote.sh                       # a cube's square face (map-cube)
#   scripts/face-and-remote.sh examples/patches/map-screen/map-screen.vy
#   scripts/face-and-remote.sh examples/patches/map-show/map-show.vy --key space=/hands
#
# The first argument is the patch; anything after it goes to `vybe run`.
# Close either window (or Ctrl-C here) and both go down together.
set -e
cd "$(dirname "$0")/.."

patch="${1:-examples/patches/map-cube/map-cube.vy}"
[ $# -gt 0 ] && shift

if [ ! -f "$patch" ]; then
    echo "no such patch: $patch" >&2
    exit 1
fi

# The remote must knock on the door the face opens: the port is the patch's own
# `out … remote <port>`.
port=$(sed -e 's/#.*//' "$patch" | sed -n 's/^[[:space:]]*out[[:space:]].*[[:space:]]remote[[:space:]]\{1,\}\([0-9]\{1,\}\).*/\1/p' | head -n 1)
if [ -z "$port" ]; then
    echo "$patch has no \`remote <port>\` on its \`out\` line, so there is nothing for the remote to talk to." >&2
    echo "    help: out window 1920x1200  keystone config/keystone.json  remote 9001" >&2
    exit 1
fi

# Build first, so the two windows open together instead of one compile apart.
cargo build -q -p vybe-cli -p vybe-remote

face="" remote=""
cleanup() {
    trap - INT TERM EXIT
    # One of the two is usually gone already — that `kill` fails, and under
    # `set -e` the failure would end the cleanup before it reached the other.
    set +e
    [ -n "$face" ] && kill "$face" 2>/dev/null
    [ -n "$remote" ] && kill "$remote" 2>/dev/null
    wait 2>/dev/null
}
trap cleanup INT TERM EXIT

./target/debug/vybe run "$patch" "$@" &
face=$!

# Give the face a moment to check the patch and open its port. If it already
# quit, its own errors are on screen — don't open a remote to nothing.
sleep 1
if ! kill -0 "$face" 2>/dev/null; then
    face=""
    exit 1
fi

./target/debug/vybe-remote "127.0.0.1:$port" &
remote=$!

echo "face: $patch   remote: 127.0.0.1:$port   (close either window to stop)"
echo "remote keys: drag a corner | TAB + arrows (SHIFT = 10 px) | G grid  W white  H gray  SPACE show | S save  R reload  0 reset"

# Whichever window closes first takes the other with it.
while kill -0 "$face" 2>/dev/null && kill -0 "$remote" 2>/dev/null; do
    sleep 1
done
