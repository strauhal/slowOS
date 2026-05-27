#!/bin/sh
# SlowOS post-image script — Pi 4 baseline
# Creates the final SD card image with boot + rootfs + data partitions.

BOARD_DIR="$(dirname $0)"

# cmdline.txt for SD card images (standalone boot).
#
# IMPORTANT FOR FUTURE DEVELOPERS:
#   - SD card / standalone boot (what you are building here):
#       Must use local root on the SD card: root=/dev/mmcblk0p2
#   - Network boot development (from gx2):
#       The cmdline is supplied via TFTP instead
#       (see config/pi4-phase2-serial-tftp-config.txt + the
#       sync-pi4-tftp-videocore-from-buildroot.sh and
#       deploy-pi4-phase2-deterministic.sh scripts).
#   The old NFS root line is ONLY valid during network-boot testing.

cat > "${RPI_FW_DIR}/cmdline.txt" << BOOTCMD
console=serial0,115200 console=tty1 root=/dev/mmcblk0p2 rootwait
BOOTCMD

# Generate SD card image
support/scripts/genimage.sh -c "${BOARD_DIR}/genimage.cfg"

echo ""
echo "============================================"
echo " SlowOS SD card image ready!"
echo " Flash with:"
echo "   dd if=output/images/sdcard.img of=/dev/sdX bs=4M"
echo " (replace /dev/sdX with your SD card device)"
echo "============================================"
