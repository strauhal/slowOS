#!/bin/sh
# Single entrypoint: NFS overlay (+ SSH perms) and optional TFTP Videocore sync.
# Panel-first Phase 2: keep rootfs + published config.txt (hdmi_cvt / KMS) in one deploy path.
# Usage (gx2): sudo ./deploy-pi4-phase2-deterministic.sh
# Optional: SLOWOS_SYNC_TFTP=1 to also run sync-pi4-tftp-videocore-from-buildroot.sh
set -eu
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd)"
ROOT="${SLOWOS_NFS_ROOT:-/nfsroot/rootfs}"
BR_TARGET_USR_BIN="$(CDPATH='' cd -- "${SCRIPT_DIR}/../buildroot/.buildroot/output/target/usr/bin" && pwd)"

"${SCRIPT_DIR}/rsync-nfs-rootfs-overlay.sh"

# Desktop shell is not in rootfs_overlay; ship a fresh binary when present.
if [ -n "${SLOWOS_SLOWDESKTOP_BIN:-}" ] && [ -f "${SLOWOS_SLOWDESKTOP_BIN}" ]; then
	cp -f "${SLOWOS_SLOWDESKTOP_BIN}" "${ROOT}/usr/bin/slowdesktop"
	chmod 755 "${ROOT}/usr/bin/slowdesktop"
	echo "OK: synced slowdesktop from SLOWOS_SLOWDESKTOP_BIN=${SLOWOS_SLOWDESKTOP_BIN}"
elif [ -f "${BR_TARGET_USR_BIN}/slowdesktop" ]; then
	cp -f "${BR_TARGET_USR_BIN}/slowdesktop" "${ROOT}/usr/bin/slowdesktop"
	chmod 755 "${ROOT}/usr/bin/slowdesktop"
	echo "OK: synced slowdesktop from Buildroot target (${BR_TARGET_USR_BIN}/slowdesktop)"
else
	echo "WARN: no slowdesktop to sync — set SLOWOS_SLOWDESKTOP_BIN=…/slowdesktop or run: cd ../buildroot/.buildroot && make slowos-rebuild"
fi

if [ "${SLOWOS_SYNC_TFTP:-0}" = 1 ]; then
	"${SCRIPT_DIR}/sync-pi4-tftp-videocore-from-buildroot.sh"
fi

echo "Deploy done. Power-cycle the Pi now (cold boot) so netboot NFS + clients load this rootfs."
echo "After boot: tail -f /var/log/eink-init.log /var/log/eink-bridge.log"
echo "Rebuild slowdesktop for this rootfs (correct libc): cd ${SCRIPT_DIR}/../buildroot/.buildroot && make slowos-rebuild"
echo "Then redeploy: sudo cp ${BR_TARGET_USR_BIN}/slowdesktop ${ROOT}/usr/bin/slowdesktop && sudo chmod 755 ${ROOT}/usr/bin/slowdesktop"
echo "Do not copy gx2 host ../target/*/release/slowdesktop onto NFS — it often needs a newer glibc than Buildroot provides."
echo "From gx2 → Pi e-ink proof (one SSH): ${SCRIPT_DIR}/pi4-eink-prove-from-gx2.sh"
echo "Remote verify: SLOWOS_PI_SSH='ssh -i … BatchMode=yes root@HOST' SLOWOS_VERIFY_EINK=1 ${SCRIPT_DIR}/verify-phase2-boot-contract.sh"
