#!/bin/sh
# Background bring-up for eink-bridge: wait for SPI + X0 + deps, then start once.
# Phase 2.5: set SLOWOS_EINK_PHASE25=0 in /etc/default/slowos-eink for legacy single-stream loop (see handoff).
# X0 is the panel-first canvas (960×680); e-ink mirrors :0, HDMI is the physical debug mirror.
# Called from S99zeink so rcS is not blocked for minutes (NFS / slow X / late spidev).
set -eu

DAEMON="/usr/bin/eink-bridge"
PIDFILE="/var/run/eink-bridge.pid"
LOGFILE="/var/log/eink-bridge.log"
INIT_LOG="/var/log/eink-init.log"
DISPLAY_VALUE="${SLOWOS_EINK_DISPLAY:-:0}"
EINK_FPS="${SLOWOS_EINK_FPS:-6}"
X_SOCKET="/tmp/.X11-unix/X0"
SPI_WAIT_SEC="${SLOWOS_EINK_SPI_WAIT_SEC:-180}"
X_WAIT_SEC="${SLOWOS_EINK_X_WAIT_SEC:-180}"
LOCK_DIR="/run/slowos-eink-launcher.lock"

zeink_log() {
	ts="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo "?")"
	echo "$ts slowos-eink-launch $*" >>"$INIT_LOG"
}

if command -v flock >/dev/null 2>&1; then
	exec 9>/run/slowos-eink-launch.flock
	flock -n 9 || { zeink_log "SKIP another launcher holds flock"; exit 0; }
else
	if ! mkdir "$LOCK_DIR" 2>/dev/null; then
		if ps w 2>/dev/null | grep -q '[s]lowos-eink-launch'; then
			zeink_log "SKIP lock busy (launcher running)"
			exit 0
		fi
		rmdir "$LOCK_DIR" 2>/dev/null || true
		mkdir "$LOCK_DIR" 2>/dev/null || { zeink_log "SKIP lock busy ($LOCK_DIR)"; exit 0; }
	fi
	trap 'rmdir "$LOCK_DIR" 2>/dev/null || true' EXIT INT TERM
fi

zeink_log "background launcher pid=$$ SPI_WAIT=${SPI_WAIT_SEC}s X_WAIT=${X_WAIT_SEC}s"

if [ -s "$PIDFILE" ]; then
	old="$(tr -d ' \t\r\n' <"$PIDFILE")"
	if [ -n "$old" ] && kill -0 "$old" 2>/dev/null; then
		zeink_log "SKIP eink-bridge already running pid=$old"
		exit 0
	fi
fi

n=0
while [ "$n" -lt "$SPI_WAIT_SEC" ]; do
	[ -c /dev/spidev0.0 ] && break
	sleep 1
	n=$((n + 1))
done
if [ ! -c /dev/spidev0.0 ]; then
	zeink_log "FAIL /dev/spidev0.0 missing after ${SPI_WAIT_SEC}s"
	exit 0
fi
zeink_log "spidev0.0 ready (${n}s)"

if ! command -v xwd >/dev/null 2>&1; then
	zeink_log "FAIL xwd missing"
	exit 0
fi

if [ ! -f /usr/lib/python3/slowos_xwd.py ]; then
	zeink_log "FAIL slowos_xwd.py missing — deploy rootfs overlay"
	exit 0
fi

n=0
while [ "$n" -lt "$X_WAIT_SEC" ]; do
	[ -S "$X_SOCKET" ] && break
	sleep 1
	n=$((n + 1))
done
if [ ! -S "$X_SOCKET" ]; then
	zeink_log "FAIL $X_SOCKET missing after ${X_WAIT_SEC}s"
	exit 0
fi
zeink_log "X0 socket ready (${n}s)"

if start-stop-daemon -S -b -m -p "$PIDFILE" \
	-x /usr/bin/python3 -- -u "$DAEMON" \
	--display "$DISPLAY_VALUE" --no-xvfb --fps "$EINK_FPS" \
	>>"$LOGFILE" 2>&1; then
	zeink_log "start-stop-daemon OK pid=$(tr -d ' \t\r\n' <"$PIDFILE" 2>/dev/null || echo ?)"
else
	zeink_log "start-stop-daemon FAIL"
fi
