#!/bin/sh
# Sample pointer position every 100 ms for latency / motion diagnostics (Phase 2).
# Usage on Pi: DISPLAY=:0 slowos-mouse-poll-100ms.sh 300 > /tmp/mouse100.log
# Move mouse while sampling; inspect jumps vs wall clock.
set -eu
OUT="${SLOWOS_MOUSE_POLL_LOG:-/tmp/mouse100.log}"
N="${1:-300}"
export DISPLAY="${DISPLAY:-:0}"
if ! command -v xdotool >/dev/null 2>&1; then
	echo "xdotool missing" >&2
	exit 1
fi
: >"$OUT"
	i=0
	while [ "$i" -lt "$N" ]; do
		# BusyBox date often lacks %N; use sample index + wall seconds for correlation.
		ts="$(date +%s 2>/dev/null || echo 0)"
		loc="$(xdotool getmouselocation 2>/dev/null || echo 'x=? y=?')"
		echo "${i} ${ts} ${loc}" >>"$OUT"
	i=$((i + 1))
	usleep 100000 2>/dev/null || sleep 0.1
done
echo "wrote $N lines to $OUT"
