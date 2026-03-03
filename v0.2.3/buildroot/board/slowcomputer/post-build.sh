#!/bin/sh
# SlowOS post-build script
# Runs after the root filesystem is assembled but before image creation.

BOARD_DIR="$(dirname $0)"
ROOTFS="$TARGET_DIR"

# Make init script executable
chmod 755 "$ROOTFS/etc/init.d/S99slowos"

# Set hostname
echo "slowbook" > "$ROOTFS/etc/hostname"

# Configure auto-login on tty1 (fallback if graphics fail)
sed -i 's|^tty1::.*|tty1::respawn:/bin/sh -l|' "$ROOTFS/etc/inittab" 2>/dev/null

# Create user directories
mkdir -p "$ROOTFS/root/documents"
mkdir -p "$ROOTFS/root/music"
mkdir -p "$ROOTFS/root/pictures"
mkdir -p "$ROOTFS/root/.config"
mkdir -p "$ROOTFS/run/user/0"

# Mount data partition at /data, symlink user dirs to it
mkdir -p "$ROOTFS/data"
grep -q "/dev/mmcblk0p3" "$ROOTFS/etc/fstab" || \
    echo "/dev/mmcblk0p3 /data ext4 defaults,noatime 0 2" >> "$ROOTFS/etc/fstab"

# Set up tmpfs for /tmp
grep -q "tmpfs.*\/tmp" "$ROOTFS/etc/fstab" || \
    echo "tmpfs /tmp tmpfs defaults,nosuid,nodev 0 0" >> "$ROOTFS/etc/fstab"

# Create a first-boot script that migrates dirs to data partition
cat > "$ROOTFS/etc/init.d/S01firstboot" << 'FIRSTBOOT'
#!/bin/sh
# First boot: set up data partition with user directories

STAMP="/data/.slowos-initialized"

if [ ! -f "$STAMP" ]; then
    echo "SlowOS first boot setup..."

    # Create directories on data partition
    mkdir -p /data/documents
    mkdir -p /data/music
    mkdir -p /data/pictures
    mkdir -p /data/config
    mkdir -p /data/trash

    # Symlink from root home to data partition
    rm -rf /root/documents /root/music /root/pictures
    ln -sf /data/documents /root/documents
    ln -sf /data/music /root/music
    ln -sf /data/pictures /root/pictures
    ln -sf /data/config /root/.config

    touch "$STAMP"
    echo "SlowOS first boot setup complete"
fi
FIRSTBOOT
chmod 755 "$ROOTFS/etc/init.d/S01firstboot"

echo "SlowOS post-build complete"
