#!/bin/sh
# SlowOS post-image script
# Creates the final SD card image with boot + rootfs + data partitions.

BOARD_DIR="$(dirname $0)"

# Generate SD card image
support/scripts/genimage.sh -c "${BOARD_DIR}/genimage.cfg"

echo ""
echo "============================================"
echo " SlowOS SD card image ready!"
echo " Flash with:"
echo "   dd if=output/images/sdcard.img of=/dev/sdX bs=4M"
echo " (replace /dev/sdX with your SD card device)"
echo "============================================"
