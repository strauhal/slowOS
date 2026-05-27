#!/bin/sh
# Phase 2 field diagnostics: boot path, X, e-ink bridge, load, slowdesktop hints.
# Run on Pi: /usr/local/sbin/slowos-phase2-diag.sh | tee /tmp/phase2-diag.txt
set -eu
export DISPLAY="${DISPLAY:-:0}"
echo "=== slowos-phase2-diag $(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo) ==="
echo "--- uptime / load ---"
uptime 2>/dev/null || true
echo "--- memory ---"
free -m 2>/dev/null || true
echo "--- top (slowdesktop, eink, Xorg) ---"
ps w 2>/dev/null | grep -E "[s]lowdesktop|[e]ink-bridge|[X]org|[x]init" || true
echo "--- xdpyinfo (root size) ---"
xdpyinfo -display "$DISPLAY" 2>/dev/null | grep -E "dimensions|resolution" || echo "(xdpyinfo failed)"
echo "--- xrandr head ---"
xrandr --display "$DISPLAY" 2>/dev/null | head -12 || true
echo "--- /run/eink-bridge.status ---"
cat /run/eink-bridge.status 2>/dev/null | sed 's/,/\n/g' | head -40 || echo "(no status)"
echo "--- slowos.log (startup_state + xinit-client + hdmi) ---"
grep -E "startup_state|slowos-xinit-client|HDMI mode|OK \(x11\)|FAIL|session_timeout" /var/log/slowos.log 2>/dev/null | tail -40 || true
echo "--- eink-bridge.log tail ---"
tail -35 /var/log/eink-bridge.log 2>/dev/null || true
echo "--- eink-init.log tail ---"
tail -15 /var/log/eink-init.log 2>/dev/null || true
echo "--- slowdesktop input telemetry tail (if enabled) ---"
tail -20 /tmp/slowdesktop-input-telemetry.log 2>/dev/null || echo "(no telemetry file)"
echo "--- xwd one-shot timing (ms) ---"
if command -v xwd >/dev/null 2>&1; then
	t0="$(date +%s)"
	xwd -root -display "$DISPLAY" -silent -out /tmp/diag.xwd 2>/dev/null && t1="$(date +%s)" && echo "xwd wall_sec=$((t1 - t0)) (BusyBox date; coarse)"
	ls -la /tmp/diag.xwd 2>/dev/null || true
	rm -f /tmp/diag.xwd
else
	echo "xwd missing"
fi
echo "--- env knobs (e-ink / child watch) ---"
echo "SLOWOS_EINK_FPS=${SLOWOS_EINK_FPS:-}"
echo "SLOWOS_EINK_CURSOR_OVERLAY=${SLOWOS_EINK_CURSOR_OVERLAY:-}"
echo "SLOWOS_EINK_MIN_PUSH_INTERVAL_SEC=${SLOWOS_EINK_MIN_PUSH_INTERVAL_SEC:-}"
echo "SLOWDESKTOP_CHILDWATCH_MS=${SLOWDESKTOP_CHILDWATCH_MS:-}"
echo "SLOWDESKTOP_INPUT_TELEMETRY=${SLOWDESKTOP_INPUT_TELEMETRY:-}"
echo "=== end ==="
