#!/bin/sh
# Event → root framebuffer change (slowOS GUI latency acceptance helper).
# Requires: DISPLAY, xwd, sha256sum or md5sum, xdotool on PATH.
#
# Usage:
#   DISPLAY=:0 ./measure-event-to-root.sh key a
#   DISPLAY=:0 ./measure-event-to-root.sh click 400 200
#
# Prints: t0 t1 delta_ms hash_before hash_after
# Exit 1 if prerequisites missing; exit 2 if root unchanged within timeout.

set -eu
MODE="${1:-}"
ARG="${2:-}"

ts_ns() {
	if command -v python3 >/dev/null 2>&1; then
		python3 -c "import time; print(int(time.time()*1e9))"
	else
		date +%s%3N 2>/dev/null || date +%s
	fi
}

hash_root() {
	if command -v sha256sum >/dev/null 2>&1; then
		DISPLAY="${DISPLAY:-:0}" xwd -root -silent 2>/dev/null | sha256sum | awk '{print $1}'
	elif command -v md5sum >/dev/null 2>&1; then
		DISPLAY="${DISPLAY:-:0}" xwd -root -silent 2>/dev/null | md5sum | awk '{print $1}'
	else
		echo ""
	fi
}

require() {
	command -v "$1" >/dev/null 2>&1 || {
		echo "missing: $1" >&2
		exit 1
	}
}

require xwd
HTEST="$(hash_root)"
[ -n "$HTEST" ] || {
	echo "xwd/hash failed (DISPLAY? permissions?)" >&2
	exit 1
}
require xdotool

H0="$(hash_root)"
T0="$(ts_ns)"

case "$MODE" in
key)
	[ -n "$ARG" ] || {
		echo "usage: $0 key <letter>" >&2
		exit 1
	}
	xdotool key --delay 0 "$ARG"
	;;
click)
	X="${2:-}"
	Y="${3:-}"
	[ -n "$X" ] && [ -n "$Y" ] || {
		echo "usage: $0 click <x> <y>" >&2
		exit 1
	}
	xdotool mousemove --sync "$X" "$Y" click 1
	;;
*)
	echo "usage: $0 key <keyname> | $0 click <x> <y>" >&2
	exit 1
	;;
esac

deadline=$(( $(date +%s) + 5 ))
H1="$H0"
while [ "$(date +%s)" -lt "$deadline" ]; do
	H1="$(hash_root)"
	[ "$H1" != "$H0" ] && break
	sleep 0.02
done
T1="$(ts_ns)"

if command -v python3 >/dev/null 2>&1; then
	DELTA_MS="$(python3 -c "print(int(($T1 - $T0) / 1e6))")"
else
	DELTA_MS="$((T1 - T0))"
fi

echo "t0=$T0 t1=$T1 delta_ms=$DELTA_MS hash0=$H0 hash1=$H1"
if [ "$H1" = "$H0" ]; then
	echo "no root change within timeout" >&2
	exit 2
fi
