#!/bin/sh
# Proof-oriented checks over SSH: Dropbear reachability, optional HW probe, runtime status JSON.
# Requires pubkey auth (run sudo ./rsync-nfs-rootfs-overlay.sh on gx2 first if Permission denied).
# Usage:
#   ./pi4-eink-remote-check.sh
#   # or set SLOWOS_PI_SSH explicitly. Defaults match pi4-eink-prove-from-gx2.sh when /tmp/pi4_root_key exists.
# Optional: SLOWOS_EINK_PROBE=1 runs slowos-eink-hw-probe (stop S99zeink first if SPI busy).
set -eu
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd)"
PI_HOST="${SLOWOS_PI_HOST:-192.168.19.138}"
PI_KEY="${SLOWOS_PI_KEY:-/tmp/pi4_root_key}"

usage() {
	echo "usage: $0   (with $PI_KEY present), or set SLOWOS_PI_SSH='ssh … root@host'" >&2
	exit 1
}

if [ -z "${SLOWOS_PI_SSH:-}" ] && [ -f "$PI_KEY" ]; then
	SLOWOS_PI_SSH="ssh -o IdentitiesOnly=yes -i ${PI_KEY} -o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=15 root@${PI_HOST}"
	export SLOWOS_PI_SSH
fi

[ -n "${SLOWOS_PI_SSH:-}" ] || usage

rsh() {
	# shellcheck disable=SC2086
	$SLOWOS_PI_SSH "$@"
}

echo "=== pi4-eink-remote-check ==="
rsh uname -a
rsh sh -c 'ls -la /root/.ssh /root/.ssh/authorized_keys 2>&1; ls -l /dev/spidev0.0 2>&1; test -S /tmp/.X11-unix/X0 && echo X0_OK || echo X0_MISSING'
echo "--- /run/eink-bridge.status (bridge writes this every diag interval) ---"
rsh sh -c 'cat /run/eink-bridge.status 2>/dev/null || echo "(no status file yet)"'
echo "--- log tails ---"
rsh sh -c 'tail -n 25 /var/log/eink-init.log 2>/dev/null || true'
rsh sh -c 'tail -n 25 /var/log/eink-bridge.log 2>/dev/null || true'

if [ "${SLOWOS_EINK_PROBE:-0}" = 1 ]; then
	echo "--- slowos-eink-hw-probe (SPI; stop eink-bridge first if SPI busy) ---"
	rsh /usr/local/bin/slowos-eink-hw-probe || true
fi

echo "=== done (run with SLOWOS_EINK_PROBE=1 for SPI hardware probe) ==="
