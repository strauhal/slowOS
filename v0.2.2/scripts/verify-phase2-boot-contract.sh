#!/bin/sh
# After a cold boot, assert HDMI+KMS+SPI+e-ink prereqs in one SSH session (same flattened DT + runtime).
# E-ink starts in background after rcS; with SLOWOS_VERIFY_EINK=1 this script polls up to
# SLOWOS_VERIFY_EINK_WAIT_SEC (default 210) before failing the process check.
# Usage:
#   ./verify-phase2-boot-contract.sh
#   # or explicit:
#   SLOWOS_PI_SSH='ssh -i /path/key -o StrictHostKeyChecking=no root@HOST' ./verify-phase2-boot-contract.sh
# Default SSH matches pi4-eink-prove-from-gx2.sh when /tmp/pi4_root_key exists.
set -eu

PI_HOST="${SLOWOS_PI_HOST:-192.168.19.138}"
PI_KEY="${SLOWOS_PI_KEY:-/tmp/pi4_root_key}"

usage() {
	echo "usage: $0   (with $PI_KEY present), or set SLOWOS_PI_SSH='ssh ... root@host'" >&2
	exit 1
}

if [ -z "${SLOWOS_PI_SSH:-}" ] && [ -f "$PI_KEY" ]; then
	SLOWOS_PI_SSH="ssh -o IdentitiesOnly=yes -i ${PI_KEY} -o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=15 root@${PI_HOST}"
	export SLOWOS_PI_SSH
fi

[ -n "${SLOWOS_PI_SSH:-}" ] || usage

fail() {
	echo "BOOT_CONTRACT_FAIL: $*" >&2
	exit 1
}

pass() {
	echo "BOOT_CONTRACT_OK: $*"
}

rsh() {
	# shellcheck disable=SC2086
	$SLOWOS_PI_SSH "$@"
}

rsh_sh() {
	rsh sh -c "$1"
}

echo "=== Phase 2 boot contract (remote checks) ==="

SPI_STATUS="$(rsh_sh 'tr -d "\0" < /proc/device-tree/soc/spi@7e204000/status 2>/dev/null || echo missing')"
if [ "$SPI_STATUS" != "okay" ]; then
	fail "spi@7e204000 status='$SPI_STATUS' (expected okay). DT path: /proc/device-tree/soc/spi@7e204000/status"
fi
pass "spi@7e204000 status=okay"

SPI_DEVS="$(rsh_sh 'ls -A /sys/bus/spi/devices 2>/dev/null | wc -l | tr -d " "')"
if [ "${SPI_DEVS:-0}" -eq 0 ] 2>/dev/null; then
	fail "/sys/bus/spi/devices is empty"
fi
pass "/sys/bus/spi/devices has entries ($SPI_DEVS)"

rsh_sh 'test -c /dev/spidev0.0' || fail "/dev/spidev0.0 missing or not a char device"
pass "/dev/spidev0.0 present"

rsh_sh 'set -- /sys/class/drm/card[0-9]*; test -e "$1"' || fail "no /sys/class/drm/card*"
VC4_FOUND="$(rsh_sh 'vc4=0; for c in /sys/class/drm/card[0-9]*; do [ -e "$c" ] || continue; t=$(readlink -f "$c" 2>/dev/null || echo ""); case "$t" in *vc4*) vc4=1; break;; esac; done; echo "$vc4"')"
if [ "$VC4_FOUND" != "1" ]; then
	fail "no DRM card symlink resolving under *vc4* (vc4-drm not exposed?). Check: ls -la /sys/class/drm/"
fi
pass "DRM exposes a vc4-backed card"

rsh_sh 'test -S /tmp/.X11-unix/X0' || fail "Xorg socket /tmp/.X11-unix/X0 missing"
pass "X11 :0 socket present"

if [ "${SLOWOS_VERIFY_EINK:-0}" = 1 ]; then
	_wait="${SLOWOS_VERIFY_EINK_WAIT_SEC:-210}"
	_n=0
	while [ "$_n" -lt "$_wait" ]; do
		if rsh_sh 'ps w | grep -q "[p]ython3.*eink-bridge"'; then
			pass "eink-bridge process present (after ${_n}s poll)"
			break
		fi
		sleep 1
		_n=$((_n + 1))
	done
	if [ "$_n" -ge "$_wait" ]; then
		fail "eink-bridge not running after ${_wait}s (see /var/log/eink-init.log on Pi; set SLOWOS_VERIFY_EINK=0 to skip)"
	fi
fi

echo "=== BOOT_CONTRACT_PASS (all gates on this boot) ==="
