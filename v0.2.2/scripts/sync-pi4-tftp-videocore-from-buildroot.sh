#!/bin/sh
# Deterministic Videocore + overlay + Phase 2 netboot config.txt for Pi 4 TFTP boot.
#
# Firmware / overlays: Buildroot rpi-firmware package extract (matches post-image.sh).
# config.txt: repo canonical only — config/pi4-phase2-serial-tftp-config.txt (KMS then SPI;
# same bytes as publish-serial-tftp-config.sh). Installs to serial prefix and /tftpboot root.
#
# Does NOT replace kernel Image/cmdline SlowOS artefacts — those are product-specific.
#
# Usage (on gx2, with tftpboot write access):
#   sudo ./sync-pi4-tftp-videocore-from-buildroot.sh
# Then cold power-cycle the Pi, then run scripts/verify-phase2-boot-contract.sh from a host with SSH.

set -eu

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd)"
V022="$(CDPATH='' cd -- "${SCRIPT_DIR}/.." && pwd)"
BUILDROOT_OUT="${BUILDROOT_OUT:-${V022}/buildroot/.buildroot/output/build}"

pick_rpi_fw_boot() {
    if [ -n "${RPI_FW_BOOT:-}" ]; then
        printf '%s' "$RPI_FW_BOOT"
        return
    fi
    # Resolve latest extracted rpi-firmware-* tree (hash in dirname changes with package bumps).
    found="$(ls -d "${BUILDROOT_OUT}"/rpi-firmware-*/boot 2>/dev/null | sort | tail -n 1)"
    if [ -n "$found" ] && [ -d "$found" ]; then
        printf '%s' "$found"
        return
    fi
    printf '%s' "${V022}/buildroot/.buildroot/output/build/rpi-firmware-unresolved/boot"
}

RPI_FW_BOOT="$(pick_rpi_fw_boot)"
TFTP_ROOT="${TFTP_ROOT:-/tftpboot}"
SERIAL_PREFIX="${SERIAL_PREFIX:-c6633b0c}"
SERIAL_DIR="${TFTP_ROOT}/${SERIAL_PREFIX}"

SLOWOS_DTBO_SRC="${V022}/device-tree/overlays/slowos-spidev-compat.dtbo"
CONFIG_SRC="${V022}/config/pi4-phase2-serial-tftp-config.txt"

die() {
    echo "error: $*" >&2
    exit 1
}

test -d "$RPI_FW_BOOT" || die "Firmware boot dir missing: $RPI_FW_BOOT (build rpi-firmware in Buildroot or set RPI_FW_BOOT)"
test -d "$SERIAL_DIR/overlays" || die "Missing serial overlays dir: $SERIAL_DIR/overlays"
test -d "${TFTP_ROOT}/overlays" || die "Missing TFTP root overlays: ${TFTP_ROOT}/overlays"
test -f "$SLOWOS_DTBO_SRC" || die "Build SlowOS overlay first: $V022/device-tree/overlays/slowos-spidev-compat.dtbo"
test -f "$CONFIG_SRC" || die "Missing canonical netboot config: $CONFIG_SRC"

for need in start4.elf fixup4.dat bcm2711-rpi-4-b.dtb; do
    test -f "${RPI_FW_BOOT}/${need}" || die "Missing ${need} under $RPI_FW_BOOT"
done
test -d "${RPI_FW_BOOT}/overlays" || die "Missing ${RPI_FW_BOOT}/overlays"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"

quarantine_serial="${SERIAL_DIR}/quarantine-videocore-sync-${stamp}"
mkdir -p "$quarantine_serial"
for pair in start4.elf fixup4.dat; do
    if [ -f "${SERIAL_DIR}/${pair}" ]; then
        cp -a "${SERIAL_DIR}/${pair}" "${quarantine_serial}/${pair}.before-sync"
    fi
done
if [ -f "${SERIAL_DIR}/config.txt" ]; then
    cp -a "${SERIAL_DIR}/config.txt" "${quarantine_serial}/config.txt.before-sync"
fi
if [ -f "${TFTP_ROOT}/config.txt" ]; then
    cp -a "${TFTP_ROOT}/config.txt" "${quarantine_serial}/config-root.before-sync"
fi

# Videocore + DTB byte-for-byte aligned with Buildroot tarball
install -o root -g root -m 755 "${RPI_FW_BOOT}/start4.elf" "${SERIAL_DIR}/start4.elf"
install -o root -g root -m 755 "${RPI_FW_BOOT}/fixup4.dat" "${SERIAL_DIR}/fixup4.dat"
install -o root -g root -m 755 "${RPI_FW_BOOT}/bcm2711-rpi-4-b.dtb" "${SERIAL_DIR}/bcm2711-rpi-4-b.dtb"

# Root TFTP (non-serial fallback)
install -o dnsmasq -g nogroup -m 755 "${RPI_FW_BOOT}/start4.elf" "${TFTP_ROOT}/start4.elf" 2>/dev/null \
    || install -o root -g root -m 755 "${RPI_FW_BOOT}/start4.elf" "${TFTP_ROOT}/start4.elf"
install -o dnsmasq -g nogroup -m 755 "${RPI_FW_BOOT}/fixup4.dat" "${TFTP_ROOT}/fixup4.dat" 2>/dev/null \
    || install -o root -g root -m 755 "${RPI_FW_BOOT}/fixup4.dat" "${TFTP_ROOT}/fixup4.dat"
install -o dnsmasq -g nogroup -m 755 "${RPI_FW_BOOT}/bcm2711-rpi-4-b.dtb" "${TFTP_ROOT}/bcm2711-rpi-4-b.dtb" 2>/dev/null \
    || install -o root -g root -m 755 "${RPI_FW_BOOT}/bcm2711-rpi-4-b.dtb" "${TFTP_ROOT}/bcm2711-rpi-4-b.dtb"

# Overlays: full tree from same package (overlay_map.dtb + vc4-* + spi* stay coherent)
rsync -a --delete "${RPI_FW_BOOT}/overlays/" "${SERIAL_DIR}/overlays/"
install -o root -g root -m 755 "$SLOWOS_DTBO_SRC" "${SERIAL_DIR}/overlays/slowos-spidev-compat.dtbo"

rsync -a --delete "${RPI_FW_BOOT}/overlays/" "${TFTP_ROOT}/overlays/"
install -o dnsmasq -g nogroup -m 755 "$SLOWOS_DTBO_SRC" "${TFTP_ROOT}/overlays/slowos-spidev-compat.dtbo" 2>/dev/null \
    || install -o root -g root -m 755 "$SLOWOS_DTBO_SRC" "${TFTP_ROOT}/overlays/slowos-spidev-compat.dtbo"

# Phase 2 netboot config (byte-identical to repo SSOT; avoids stale TFTP config after git pull)
install -o root -g root -m 644 "$CONFIG_SRC" "${SERIAL_DIR}/config.txt"
cmp -s "$CONFIG_SRC" "${SERIAL_DIR}/config.txt" || die "cmp failed: ${SERIAL_DIR}/config.txt must match canonical"
install -o dnsmasq -g nogroup -m 644 "$CONFIG_SRC" "${TFTP_ROOT}/config.txt" 2>/dev/null \
    || install -o root -g root -m 644 "$CONFIG_SRC" "${TFTP_ROOT}/config.txt"
cmp -s "$CONFIG_SRC" "${TFTP_ROOT}/config.txt" || die "cmp failed: ${TFTP_ROOT}/config.txt must match canonical"

echo "OK: Videocore + DTB + overlays + config.txt synced from:"
echo "  $RPI_FW_BOOT"
echo "Into:"
echo "  $SERIAL_DIR"
echo "  ${TFTP_ROOT}/ (root + overlays)"
echo "Quarantine:"
echo "  $quarantine_serial"
echo "--- sha256 verification bundle (deterministic fingerprint) ---"
sha256sum \
    "${SERIAL_DIR}/start4.elf" \
    "${SERIAL_DIR}/fixup4.dat" \
    "${SERIAL_DIR}/bcm2711-rpi-4-b.dtb" \
    "${SERIAL_DIR}/config.txt" \
    "${CONFIG_SRC}" \
    "${RPI_FW_BOOT}/start4.elf" \
    "${RPI_FW_BOOT}/fixup4.dat" \
    "${RPI_FW_BOOT}/bcm2711-rpi-4-b.dtb"
