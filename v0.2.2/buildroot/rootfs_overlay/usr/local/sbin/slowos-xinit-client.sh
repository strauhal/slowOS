#!/bin/sh
# Phase 2 xinit client: force HDMI to panel size before slowdesktop (EDID may prefer 4K).
# Wayland / X-reuse paths invoke /usr/bin/slowdesktop directly — not this script.

SLOWOS_LOG=/var/log/slowos.log

slowos_log() {
    echo "slowos-xinit-client: $*" >>"$SLOWOS_LOG" 2>/dev/null || true
}

W=960
H=680
if [ -r /etc/default/slowos-panel ]; then
    # shellcheck disable=SC1091
    . /etc/default/slowos-panel
fi
W="${SLOWOS_PANEL_WIDTH:-960}"
H="${SLOWOS_PANEL_HEIGHT:-680}"
MODE="${W}x${H}"
export DISPLAY="${DISPLAY:-:0}"

i=0
while [ "$i" -lt 80 ]; do
    [ -S /tmp/.X11-unix/X0 ] && break
    i=$((i + 1))
    sleep 0.05
done
if [ ! -S /tmp/.X11-unix/X0 ]; then
    slowos_log "no X0 socket before panel xrandr; exec slowdesktop"
    exec /usr/bin/slowdesktop "$@"
fi

settle="${SLOWOS_HDMI_CLIENT_SETTLE_SEC:-1}"
case "$settle" in ''|*[!0-9]*) settle=1;; esac
[ "$settle" -gt 0 ] 2>/dev/null && sleep "$settle"

if [ ! -x /usr/bin/xrandr ]; then
    slowos_log "xrandr missing; exec slowdesktop"
    exec /usr/bin/slowdesktop "$@"
fi

query_out="$(DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-}" /usr/bin/xrandr --query 2>&1)" || query_out=""
output="$(printf '%s\n' "$query_out" | awk '$1 ~ /^HDMI/ && $2 == "connected" { print $1; exit }')"

if [ -z "$output" ]; then
    slowos_log "no connected HDMI; exec slowdesktop"
    exec /usr/bin/slowdesktop "$@"
fi

if DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-}" /usr/bin/xrandr --output "$output" --mode "$MODE" >>"$SLOWOS_LOG" 2>&1; then
    slowos_log "pre-desktop hdmi: $output $MODE"
    exec /usr/bin/slowdesktop "$@"
fi

MODE_NAME="slowos_${MODE}"
modeline=""
if command -v cvt >/dev/null 2>&1; then
    modeline="$(cvt "$W" "$H" 60 2>/dev/null | sed -n 's/^Modeline //p' | head -n 1)"
    if [ -n "$modeline" ]; then
        MODE_NAME="$(printf %s "$modeline" | cut -d'"' -f2)"
    fi
fi
if [ -z "$modeline" ] && [ "$MODE" = "960x680" ]; then
    MODE_NAME="slowos_960x680"
    modeline="${MODE_NAME} 50.50 960 1016 1112 1272 680 683 693 717 -hsync +vsync"
fi
if [ -n "$modeline" ]; then
    DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-}" /usr/bin/xrandr --delmode "$output" "${MODE_NAME}" >>"$SLOWOS_LOG" 2>&1 || true
    DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-}" /usr/bin/xrandr --rmmode "${MODE_NAME}" >>"$SLOWOS_LOG" 2>&1 || true
    DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-}" /usr/bin/xrandr --newmode ${modeline} >>"$SLOWOS_LOG" 2>&1 || true
    DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-}" /usr/bin/xrandr --addmode "$output" "${MODE_NAME}" >>"$SLOWOS_LOG" 2>&1 || true
    if DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-}" /usr/bin/xrandr --output "$output" --mode "${MODE_NAME}" >>"$SLOWOS_LOG" 2>&1; then
        slowos_log "pre-desktop hdmi: $output ${MODE_NAME}"
    else
        slowos_log "pre-desktop hdmi: addmode/activate failed for ${MODE_NAME}"
    fi
else
    slowos_log "pre-desktop hdmi: no modeline for $MODE"
fi

exec /usr/bin/slowdesktop "$@"
