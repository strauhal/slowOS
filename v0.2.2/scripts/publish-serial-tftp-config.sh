#!/bin/sh
# Publish repo canonical Pi4 Phase 2 netboot config to TFT serial-prefix config.txt (requires write access).
# Usage:
#   SLOWOS_TFTP_DEST=/tftpboot/<serial>/config.txt [sudo -E] ./publish-serial-tftp-config.sh
# Optional:
#   SLOWOS_PUBLISH_IMAGE=/tftpboot/<serial>/Image  — print sha256 after successful publish
set -eu
SCRIPT_DIR="$(dirname "$0")"
REPO_ROOT="$(readlink -f "${SCRIPT_DIR}/..")"
SRC="${REPO_ROOT}/config/pi4-phase2-serial-tftp-config.txt"
DEST="${SLOWOS_TFTP_DEST:-}"

if ! [ -f "$SRC" ]; then
	echo "missing canonical config: $SRC" >&2
	exit 1
fi
if [ -z "$DEST" ]; then
	echo "set SLOWOS_TFTP_DEST to tftpboot serial config.txt path" >&2
	exit 1
fi

cp -f "$SRC" "$DEST" || {
	echo "publish: cp failed -> $DEST" >&2
	exit 1
}
chmod 644 "$DEST" 2>/dev/null || true

if ! cmp -s "$SRC" "$DEST"; then
	echo "publish: cmp failed; $DEST does not match $SRC" >&2
	exit 1
fi

echo "OK: $DEST byte-matches $(basename "$SRC")"
if command -v stat >/dev/null 2>&1; then
	echo "publish: dest mtime: $(stat -c '%y %n' "$DEST" 2>/dev/null || stat -f '%Sm %N' "$DEST")"
fi

if [ -n "${SLOWOS_PUBLISH_IMAGE:-}" ]; then
	if [ -f "$SLOWOS_PUBLISH_IMAGE" ]; then
		if command -v sha256sum >/dev/null 2>&1; then
			echo "publish: sha256 Image:"
			sha256sum "$SLOWOS_PUBLISH_IMAGE"
		else
			echo "publish: sha256sum not available; set SLOWOS_PUBLISH_IMAGE skipped" >&2
		fi
	else
		echo "publish: SLOWOS_PUBLISH_IMAGE not a file: $SLOWOS_PUBLISH_IMAGE" >&2
		exit 1
	fi
fi
