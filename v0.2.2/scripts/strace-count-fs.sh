#!/bin/sh
# Summarize filesystem-related syscalls from an strace log (offline).
# Capture on device:
#   strace -f -tt -T -p "$(pidof slowdesktop)" -o /tmp/slowdesktop.strace
# Then:
#   ./strace-count-fs.sh /tmp/slowdesktop.strace
#
# Not a substitute for full latency analysis; use with measure-event-to-root.sh timing.

set -eu
LOG="${1:-}"
[ -n "$LOG" ] && [ -f "$LOG" ] || {
	echo "usage: $0 /path/to/strace.log" >&2
	exit 1
}

for sym in getdents64 stat openat open access faccessat readlink lstat; do
	n="$(grep -c "$sym(" "$LOG" 2>/dev/null || true)"
	printf '%s %s\n' "$sym" "${n:-0}"
done

echo "--- top 20 syscalls by line count ---"
sed -n 's/.* \([a-z_0-9]*\)(.*/\1/p' "$LOG" | sort | uniq -c | sort -nr | head -20
