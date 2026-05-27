#!/bin/sh
# gx2 → Pi: one command, one SSH session. No manual steps on the Pi.
# Measures whether epd.display() actually blocks on BUSY (real refresh vs silent no-op).
#
# Usage (on gx2):
#   ./pi4-eink-prove-from-gx2.sh
# Optional:
#   SLOWOS_PI_HOST=192.168.19.138 SLOWOS_PI_KEY=/tmp/pi4_root_key ./pi4-eink-prove-from-gx2.sh
set -eu

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd)"
PI_HOST="${SLOWOS_PI_HOST:-192.168.19.138}"
PI_KEY="${SLOWOS_PI_KEY:-/tmp/pi4_root_key}"

if [ ! -f "$PI_KEY" ]; then
	echo "missing key $PI_KEY (set SLOWOS_PI_KEY)" >&2
	exit 1
fi

echo "gx2 → root@${PI_HOST}: running slowos-eink-prove (this takes ~1–2 minutes) …"

exec ssh \
	-o IdentitiesOnly=yes \
	-i "$PI_KEY" \
	-o StrictHostKeyChecking=no \
	-o BatchMode=yes \
	-o ConnectTimeout=15 \
	root@"$PI_HOST" \
	'exec env SLOWOS_EINK_PROVE_SETTLE_SEC="${SLOWOS_EINK_PROVE_SETTLE_SEC:-22}" python3 -B -u /usr/local/bin/slowos-eink-prove'
