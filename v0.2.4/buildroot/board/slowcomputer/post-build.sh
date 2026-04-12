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

# Create user directories on rootfs (fallback in case data partition
# is unavailable or currently exported over USB)
mkdir -p "$ROOTFS/root/Documents"
mkdir -p "$ROOTFS/root/Music"
mkdir -p "$ROOTFS/root/Music/MIDI"
mkdir -p "$ROOTFS/root/Pictures"
mkdir -p "$ROOTFS/root/Books"
mkdir -p "$ROOTFS/root/.config"
mkdir -p "$ROOTFS/run/user/0"

# Mount data partition (FAT32) at /data. FAT32 so the partition can be
# exposed directly over USB mass storage without needing ext4 drivers
# on the host.
mkdir -p "$ROOTFS/data"
grep -q "/dev/mmcblk0p3" "$ROOTFS/etc/fstab" || \
    echo "/dev/mmcblk0p3 /data vfat defaults,noatime,umask=0000,utf8 0 0" >> "$ROOTFS/etc/fstab"

# Set up tmpfs for /tmp
grep -q "tmpfs.*\/tmp" "$ROOTFS/etc/fstab" || \
    echo "tmpfs /tmp tmpfs defaults,nosuid,nodev 0 0" >> "$ROOTFS/etc/fstab"

# Create a first-boot script that populates the data partition with
# user directories. Because /data is now FAT32, we can't use symlinks
# (FAT32 doesn't support them) — instead, the system bind-mounts the
# data partition folders onto /root/Books, /root/Music, etc.
cat > "$ROOTFS/etc/init.d/S01firstboot" << 'FIRSTBOOT'
#!/bin/sh
# First boot: create folder structure on the data partition, then
# bind-mount each folder onto the corresponding /root/<Folder>.

STAMP="/data/.slowos-initialized"

# Create user folder structure on the FAT32 data partition
mkdir -p /data/Books
mkdir -p /data/Music
mkdir -p /data/Music/MIDI
mkdir -p /data/Pictures
mkdir -p /data/Documents
mkdir -p /data/.slowos/trash
mkdir -p /data/.slowos/config

# Ensure the mount points on rootfs exist (already created in post-build
# but defensive here for field rebuilds)
mkdir -p /root/Books /root/Music /root/Music/MIDI /root/Pictures /root/Documents /root/.config

# Bind-mount the data-partition folders onto the user's home. This is
# what makes /root/Books etc. point at real files on the FAT32 partition
# without using symlinks (which FAT32 can't represent).
mountpoint -q /root/Books      || mount --bind /data/Books      /root/Books
mountpoint -q /root/Music      || mount --bind /data/Music      /root/Music
mountpoint -q /root/Pictures   || mount --bind /data/Pictures   /root/Pictures
mountpoint -q /root/Documents  || mount --bind /data/Documents  /root/Documents
mountpoint -q /root/.config    || mount --bind /data/.slowos/config /root/.config

if [ ! -f "$STAMP" ]; then
    # First boot: drop a README onto the data partition so users who
    # plug the device in for the first time see a clear hint
    cat > /data/README.txt << 'ENDREADME'
slowBook Library
================
Drop files into these folders from your computer or phone:

  Books/      .epub .pdf .mobi .cbz .cbr
  Music/      .mp3 .wav .flac .ogg .m4a
  Music/MIDI/ .mid .midi
  Pictures/   .png .jpg .gif .bmp .webp .svg
  Documents/  .txt .html .md .rtf

Files dropped at the top level will be sorted into the correct
folder automatically when you disconnect.
ENDREADME
    touch "$STAMP"
fi
FIRSTBOOT
chmod 755 "$ROOTFS/etc/init.d/S01firstboot"

echo "SlowOS post-build complete"
