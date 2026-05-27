#!/bin/sh
# Deterministic NFS root deploy: overlay + Dropbear-required SSH permissions.
# Usage (gx2): sudo ./rsync-nfs-rootfs-overlay.sh
set -eu
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd)"
OVERLAY="$(CDPATH='' cd -- "${SCRIPT_DIR}/../buildroot/rootfs_overlay" && pwd)"
ROOT="${SLOWOS_NFS_ROOT:-/nfsroot/rootfs}"

test -d "$OVERLAY" || { echo "missing overlay $OVERLAY" >&2; exit 1; }
test -d "$ROOT" || { echo "missing NFS root $ROOT (set SLOWOS_NFS_ROOT)" >&2; exit 1; }

rsync -a "${OVERLAY}/" "${ROOT}/"

# Dropbear pubkey auth: ~/.ssh/authorized_keys must be tight AND root's home must be owned by root (not build UID).
if [ "$(id -u)" -eq 0 ]; then
	chown root:root "${ROOT}/root" 2>/dev/null || true
	chmod 700 "${ROOT}/root" 2>/dev/null || true
fi
if [ -f "${ROOT}/root/.ssh/authorized_keys" ]; then
	chmod 700 "${ROOT}/root/.ssh"
	chmod 600 "${ROOT}/root/.ssh/authorized_keys"
	if [ "$(id -u)" -eq 0 ]; then
		chown root:root "${ROOT}/root/.ssh" "${ROOT}/root/.ssh/authorized_keys"
	fi
fi
if [ -f "${ROOT}/usr/local/sbin/slowos-eink-launch.sh" ]; then
	chmod 755 "${ROOT}/usr/local/sbin/slowos-eink-launch.sh"
fi
if [ -f "${ROOT}/usr/local/bin/slowos-eink-hw-probe" ]; then
	chmod 755 "${ROOT}/usr/local/bin/slowos-eink-hw-probe"
fi
if [ -f "${ROOT}/usr/local/bin/slowos-eink-demo" ]; then
	chmod 755 "${ROOT}/usr/local/bin/slowos-eink-demo"
fi
if [ -f "${ROOT}/usr/local/bin/slowos-eink-prove" ]; then
	chmod 755 "${ROOT}/usr/local/bin/slowos-eink-prove"
fi
if [ -f "${ROOT}/etc/init.d/S99zeink" ]; then
	chmod 755 "${ROOT}/etc/init.d/S99zeink"
fi
if [ -f "${ROOT}/etc/default/slowos-panel" ]; then
	chmod 644 "${ROOT}/etc/default/slowos-panel"
fi

echo "OK: overlay -> ${ROOT} (ssh + e-ink launcher perms fixed where present)"
