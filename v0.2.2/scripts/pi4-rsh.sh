#!/bin/sh
# Deterministic gx2 → Pi root SSH (same flags as pi4-eink-prove-from-gx2.sh).
# Usage:
#   ./pi4-rsh.sh 'hostname; cat /run/eink-bridge.status'
# Optional: SLOWOS_PI_HOST=… SLOWOS_PI_KEY=… ./pi4-rsh.sh 'uptime'
set -eu
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd)"
PI_HOST="${SLOWOS_PI_HOST:-192.168.19.138}"
PI_KEY="${SLOWOS_PI_KEY:-/tmp/pi4_root_key}"
if [ ! -f "$PI_KEY" ]; then
	echo "pi4-rsh: missing key $PI_KEY (set SLOWOS_PI_KEY or install key to /tmp/pi4_root_key)" >&2
	exit 1
fi
exec ssh \
	-o IdentitiesOnly=yes \
	-i "$PI_KEY" \
	-o StrictHostKeyChecking=no \
	-o BatchMode=yes \
	-o ConnectTimeout=15 \
	root@"$PI_HOST" \
	"$@"
