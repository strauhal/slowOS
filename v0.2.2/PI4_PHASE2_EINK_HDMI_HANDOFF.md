# Pi 4 Phase 2 — E-Ink first, HDMI mirror (debug)
> **Note for readers reproducing this work elsewhere**
>
> This document describes work done in a specific development environment (gx2 + Raspberry Pi 4 on a private 192.168.19.x LAN).
> Many example commands and paths below contain concrete values from that setup (e.g. `192.168.19.138`, `192.168.19.11`, `/tmp/pi4_root_key`).
>
> These are **not** required. When working elsewhere:
> - Override the Pi address and SSH key using the environment variables `SLOWOS_PI_HOST` and `SLOWOS_PI_KEY`.
> - See `scripts/pi4-rsh.sh` and `scripts/deploy-pi4-phase2-deterministic.sh` for how the scripts consume these variables.
>
> The core technical requirements (panel-first 960×680, SPI bring-up order, S99slowos + S99zeink behavior, eink-bridge against DISPLAY=:0, etc.) remain the same regardless of the specific IPs or machine names used during development.


## gx2 → Pi (single command)

From the gx2 checkout: **`cd slowos-build/v0.2.2/scripts && ./pi4-eink-prove-from-gx2.sh`** — one SSH session runs stop/settle, paints bars, **forces a full-refresh time pad** (BUSY is unreliable here), holds, restarts `S99zeink`. Logs show **`display_wall_seconds≈18`** when the pad is active; that is the measurable proof gx2 can get without a camera on the panel.

## Mission

**E-ink is the first-class display:** SlowOS and Xorg use the **same logical resolution and aspect as the panel** (Waveshare 13.3" K: **960×680**). The **HDMI output mirrors that canvas** for SSH/debug, keyboard/mouse bring-up, and operator visibility — HDMI must **not** define a separate 16∶9 “truth” that SlowOS is then scaled down from.

Phase 2 still captures **`DISPLAY=:0`** (live Xorg, not Xvfb `:99`) for `eink-bridge`; do not route the desktop through Xvfb for this sprint unless a measured blocker proves it is required.

**Phase 2.5 (orchestrator handoff):** e-ink responsiveness, pointer-first partials, 3s anti-stale reconcile, and failure escalation — see **`PI4_PHASE2.5_EINK_QOL_HANDOFF.md`**.

## Non-Goals

- Do not expand scope to Pi Zero or Pi Zero 2 W bring-up.
- Do not start a Rust e-ink rewrite.
- Do not rebuild the graphics stack unless the SPI or capture path proves it is unavoidable.

## Display contract (panel-first)

| Layer | Role |
|--------|------|
| **Panel (e-ink)** | **960×680** — canonical pixel grid and aspect. |
| **`config/pi4-phase2-serial-tftp-config.txt`** | Firmware hint: **`hdmi_group=2`**, **`hdmi_mode=87`**, **`hdmi_cvt=960 680 60 1 0 0 1`** so KMS/HDMI timing matches the panel class. Publish to TFTP with **`scripts/sync-pi4-tftp-videocore-from-buildroot.sh`** (or **`SLOWOS_SYNC_TFTP=1`** on **`scripts/deploy-pi4-phase2-deterministic.sh`**). |
| **`/etc/default/slowos-panel`** | **`SLOWOS_PANEL_WIDTH` / `SLOWOS_PANEL_HEIGHT`** (default **960** / **680**). Single source for init + desktop unless overridden. |
| **`S99slowos`** | Sources that default, exports **`SLOWOS_VIEWPORT_W` / `H`** from the panel size, runs **`configure_hdmi_mode()`** to set **`xrandr`** to **WxH** (or **`cvt` / embedded modeline** if EDID lacks the mode). |
| **`slowdesktop`** | Default viewport **960×680** (overridable with **`SLOWOS_VIEWPORT_W` / `H`**). Layout targets the **e-ink** canvas; HDMI shows the same. |
| **`eink-bridge`** | **`xwd -root`** on **`:0`**. If capture is already **960×680**, **1:1** to the driver (no resize). If not (misconfig / fallback), **`SLOWOS_EINK_FIT`** (**`letterbox`** default, **`cover`** optional) resamples. **Dirty rectangles** (default **`SLOWOS_EINK_DIRTY_RECT=1`**) use a PIL diff vs the last pushed frame and call **`display_Partial`** on a small aligned bbox when change area is below **`SLOWOS_EINK_DIRTY_AREA_MAX`** (~0.42); otherwise one full-panel partial. **`SLOWOS_EINK_CURSOR_OVERLAY=1`** (default) draws a fat ring via **`xdotool getmouselocation`** before diff so the pointer survives 1-bit. **`/run/eink-bridge.status`** includes **`dirty_partial_count`**, **`dirty_fullrect_partial_count`**, **`fit_mode`**, and errors. |
| **HDMI monitor** | Physical mirror of the **same** framebuffer; some monitors reject non-EDID timings — that is a **bench hardware** issue, not a reason to make 1080p the software spine. |

> **Note:** The SSH examples and paths in this section reflect the development setup on gx2.
> Use the environment variables `SLOWOS_PI_HOST` and `SLOWOS_PI_KEY` (see `scripts/pi4-rsh.sh`) when working on different hardware or networks.

## Access

- Pi SSH (design path; **IdentitiesOnly** avoids ssh-agent sending other keys first):  
  `ssh -o IdentitiesOnly=yes -i /tmp/pi4_root_key -o StrictHostKeyChecking=no -o BatchMode=yes root@192.168.19.138`
- Root password (interactive terminals only): `0917`

### When SSH “stops working” (observability / agents)

Common issues (often confused):

1. **Host key rejected by Dropbear**  
   Verbose client shows: pubkey is **offered**, then **`Authentications that can continue: publickey,password`** again — meaning **the Pi did not accept this key** (wrong `authorized_keys`, reflashed rootfs, or different Pi).  
   **Expected ed25519 public key** (must appear in root’s authorized_keys on the Pi):

   ```
   ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAPD2gCZnezyLHvrA4a5O9yCsTFMkk8QG95tY8cV744c advented@Advented.local
   ```

   Restore access: rootfs ships **`/root/.ssh/authorized_keys`** (Dropbear). Re-deploy overlay with **`sudo scripts/rsync-nfs-rootfs-overlay.sh`** (fixes **`chown root:root /root`**, **`.ssh` perms**, and **`chmod 755 /etc/init.d/S99zeink`**), or fix on console: **`chown root:root /root`** (**required**), **`chmod 700 /root`**, **`chmod 700 /root/.ssh`**, **`chmod 600 /root/.ssh/authorized_keys`**.

2. **`/root` owned by uid 1000 on the NFS server**  
   Dropbear matches OpenSSH-style strict checks: root’s home must not look like a multi-user-writable tree owned by a non-root UID. If **`ls -ld /root`** on the Pi shows **`1000`** or group/world-writable modes, pubkey auth fails even when **`authorized_keys`** is correct. **`sudo rsync-nfs-rootfs-overlay.sh`** forces **`root:root`** + **`700`** on **`${SLOWOS_NFS_ROOT}/root`**.

3. **Cursor / CI / agent shells have no TTY**  
   If pubkey fails, OpenSSH falls back to **password** and tries **`read_passphrase` → `/dev/tty`** — in batch/agent contexts that fails with **`can't open /dev/tty`**, so **password auth cannot complete unattended**. This is **not** “the project forgot SSH”; it is an **environment limit**. Fix is always **restore pubkey trust** or run SSH from an **interactive** terminal with `-o BatchMode=no`.

### Waveshare **E-Paper Driver HAT** DIP baseline (project)

Use the **[E-Paper Driver HAT](https://www.waveshare.com/wiki/E-Paper_Driver_HAT)** wiki as the driver-board source of truth (not text-only dumps of figure-heavy pages).

- **Display config A** (table: resistor position **3R (A)**): includes **13.3inch e-Paper** and **13.3inch e-Paper (B)** in Waveshare’s published **Display Config** table — matches a **13.3"** panel on this HAT when set to **A**.
- **Interface config 0**: selects **4-wire SPI** (separate **DC** line). SlowOS **`epdconfig.py`** uses BCM **DC** + **`SPI.open(0, 0)`** (**CE0 → `/dev/spidev0.0`**). **Interface 1** would be **3-wire SPI** (different wiring/protocol); **do not use 1** unless the stack is rewritten for that mode.
- Panel-specific wiki + schematic: [13.3inch e-Paper HAT (K) Manual](https://www.waveshare.com/wiki/13.3inch_e-Paper_HAT_(K)_Manual), [Driver HAT schematic PDF](https://files.waveshare.com/wiki/13.3inch-e-Paper-HAT-(K)/E-Paper-Driver-HAT-Schematic.pdf).

**FAQ (SPI busy / spidev):** if another DT node occupies **SPI0 CS0**, Waveshare demos may use **`SPI.open(0, 1)`**. SlowOS stays on **`open(0, 0)`** until CE1 is proven required.

## Deterministic boot procedure (gx2 → Pi)

Order matters; skipping steps produces “random” HDMI-only or SSH failures.

1. **`git pull`** this repo on gx2 (overlay + scripts are the contract).
2. **Rootfs to NFS:** `cd …/v0.2.2/scripts && sudo ./rsync-nfs-rootfs-overlay.sh`  
   (sets **`/root/.ssh/authorized_keys`** perms; override NFS root with **`SLOWOS_NFS_ROOT`** if needed).
3. **TFTP + panel `config.txt`:** `sudo ./sync-pi4-tftp-videocore-from-buildroot.sh` when **`config/pi4-phase2-serial-tftp-config.txt`** ( **`hdmi_cvt`** / KMS hints), **`Image`**, or overlays change — or run **`sudo SLOWOS_SYNC_TFTP=1 ./deploy-pi4-phase2-deterministic.sh`** so NFS + TFTP stay in one step.
4. **Cold power-cycle** the Pi (DT + `/boot` firmware state).
5. **Verify:** `ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no -o BatchMode=yes root@192.168.19.138 hostname`
6. **E-ink:** `tail -40 /var/log/eink-init.log /var/log/eink-bridge.log` on the Pi.

## gx2 / Network Paths
> **Note:** The paths below (e.g. `/home/advented-gx2/slowos-build/v0.2.2`, `/nfsroot/rootfs`) are specific to the development machine (gx2). When working on your own setup, adjust `SLOWOS_NFS_ROOT` and work from your local checkout of this repo.

- NFS export path (typical): `/nfsroot/rootfs` — deploy **only** via **`scripts/rsync-nfs-rootfs-overlay.sh`** or equivalent **`rsync -a` + chmod** on **`/root/.ssh/`** (git checkout files are often mode **644**; Dropbear requires **600**).
- TFTP boot path: `/tftpboot`
- Pi 4 serial-scoped TFTP config: `/tftpboot/c6633b0c/config.txt` is the hot-fix target copied from `/tftpboot/config.txt`
- NFS root path: `/nfsroot/rootfs`
- v0.2.2 source: `/home/advented-gx2/slowos-build/v0.2.2`
- Buildroot live config: `/home/advented-gx2/slowos-build/v0.2.2/buildroot/.buildroot/.config`
- Pi 4 defconfig: `/home/advented-gx2/slowos-build/v0.2.2/buildroot/configs/slowos_pi4_defconfig`
- Buildroot output: `/home/advented-gx2/slowos-build/v0.2.2/buildroot/.buildroot/output`
- Rootfs overlay source of truth: `/home/advented-gx2/slowos-build/v0.2.2/buildroot/rootfs_overlay`

## Phase 1 Source Of Truth

Latest patches are in the source overlay and deployed NFS root. Treat those as the current handoff authority; do not assume the Buildroot output has been rebuilt with these changes.

- NFS-root network guard: `/etc/network/nfs_check` prevents DHCP from dropping the interface backing NFS root; `/etc/network/interfaces` runs it before interface bring-up.
- **Xorg `:0` (panel-first):** `S99slowos` starts SlowOS on Xorg `:0`, configures HDMI to the **panel size** (**960×680** by default via **`/etc/default/slowos-panel`** + **`xrandr`**), disables Wayland by default through `SLOWOS_ENABLE_WAYLAND=0`, and enables input telemetry by default. **HDMI mirrors that canvas** for debug, not a separate 1080p layout.
- E-ink autostart **on by default**: `S99zeink` runs **after** `S99slowos`. Disable with `SLOWOS_ENABLE_EINK=0`. Requires `/dev/spidev0.0`, `/usr/lib/python3/slowos_xwd.py`, `xwd`, and Xorg `:0` (`/tmp/.X11-unix/X0`). Starts `python3 -u /usr/bin/eink-bridge --display :0 --no-xvfb`. Capture is **root `xwd` → `slowos_xwd`** (ImageMagick is **not** used). When X is **960×680**, the bridge path is **1:1** to the panel; otherwise **`SLOWOS_EINK_FIT`** controls resampling. Diagnostics: **`tail -f /var/log/eink-bridge.log`** (`diag:` lines), **`cat /run/eink-bridge.status`**.
- `S99slowos` **gates** X11: requires `/dev/fb0` **or** a DRM card whose driver name contains `vc4` (KMS HDMI). It writes `/etc/X11/xorg.conf.d/42-slowos-vc4-kmsdev.conf` to pin `modesetting` to that card so X does not attach to `v3d`-only `card0`. Stops `S99zeink` before tearing down X.
- **Primary deploy (config + firmware + overlays):** on gx2, `sudo /path/to/slowos-build/v0.2.2/scripts/sync-pi4-tftp-videocore-from-buildroot.sh` (writes canonical `config/pi4-phase2-serial-tftp-config.txt` to `/tftpboot/<serial>/config.txt` and `/tftpboot/config.txt`, `cmp` internally). Override `TFTP_ROOT`, `SERIAL_PREFIX`, `BUILDROOT_OUT`, `RPI_FW_BOOT` if needed.
- **Config-only publish:** `SLOWOS_TFTP_DEST=/tftpboot/<serial>/config.txt sudo -E …/scripts/publish-serial-tftp-config.sh`
- Boot slowdesktop focus hardening is patched in `S99slowos`: it waits for Xorg readiness, verifies input, claims slowdesktop focus, logs focus telemetry, and keeps strict behavior opt-in.
- Launched app focus handoff remains a known bare-Xorg issue. Do not treat Rust app launch focus improvements as completed; for example, a `slownotes` window can still need explicit focus after launch.

## Live Findings

- Keyboard and mouse work after the slowdesktop focus claim succeeds.
- Remaining bare-Xorg app focus handoff and per-app focus latency are not fully solved.
- `slownotes` still has a window focus issue that needs direct validation during Phase 2.
- No window manager is installed; focus behavior is therefore dependent on app windows, Xorg, `xdotool`, and `wmctrl` behavior rather than WM policy.
- SPI: `slowos-spidev-compat` + `dtparam=spi=on`; **order matters:** apply `vc4-kms-v3d-pi4,noaudio` first, then **second** `dtparam=spi=on`, then `slowos-spidev-compat` (KMS merge can leave `spi@7e204000` **disabled** — no `/dev/spidev*`, empty `/sys/bus/spi/devices/`). Power-cycle after TFTP edits; validate **`cat /proc/device-tree/soc/spi@7e204000/status`** is **okay** and **`ls /dev/spidev*`** exists.
- KMS: TFTP uses `dtoverlay=vc4-kms-v3d-pi4,noaudio` **without** `dtparam=audio=on` until HDMI/X is validated. SlowOS linux.config enables **`CONFIG_SND_BCM2835=y`** (legacy driver was `# not set`; without it **`vc4_hdmi` PCM stalls at `-517`**, **`vc4-drm` never registers/display `card*`**, only **`v3d`**). After changing kernel config **rebuild** Buildroot/Linux and redeploy **`Image`** under `/tftpboot/<serial>/`. Do **not** use `vc4-fkms-v3d` on this baseline (Oops risk).
- `/proc/cmdline` may prepend `snd_bcm2835.*` and `video=…` beyond `cmdline.txt`; treat TFT `cmdline.txt` as partial—always read `/proc/cmdline` on the Pi when triaging display/audio.

## Phase 2 Execution Order

1. Prove Pi 4 still boots to **Xorg `:0`** at **panel resolution** (see **`DISPLAY=:0 xrandr --query`** / **`xdpyinfo`**) from the current NFS root + published **`config.txt`**.
2. Fix SPI exposure until `/dev/spidev0.0` exists on the Pi.
3. Start e-ink capture against **the same `:0` canvas** (not Xvfb `:99`).
4. Validate that the **physical HDMI output** remains usable as a **mirror** for SSH/debug while `eink-bridge` runs (some monitors may not like **960×680** — try another display or adjust **`hdmi_cvt`** only after confirming software contract on **`xdpyinfo`**).
5. Only then address app focus latency, starting with `slownotes`.

## Exact Validation Commands

Run from gx2:

```bash
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'hostname; uptime; mount | grep " on / "; ip addr; ip route'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'cat /proc/cmdline'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'ls -la /sys/class/drm/; readlink -f /sys/class/drm/card0 2>/dev/null || true'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'ls -l /dev/spidev0.0 /dev/fb0 /dev/dri/card0 2>&1 || true'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'dmesg | grep -iE "vc4_hdmi|PCM|oops|bug:" | tail -30'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'test -S /tmp/.X11-unix/X0 && echo X0 socket present || echo X0 socket missing'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'DISPLAY=:0 xrandr --query'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'DISPLAY=:0 xinput list --short'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'DISPLAY=:0 xdotool getwindowfocus getwindowname'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'ps w | grep -E "Xorg|xinit|slowdesktop|eink|Xvfb" | grep -v grep'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'tail -n 120 /var/log/slowos.log /var/log/eink-bridge.log 2>/dev/null'
ssh -o IdentitiesOnly=yes -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'tail -n 80 /var/log/eink-init.log 2>/dev/null'
ssh -o IdentitiesOnly=yes -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'cat /run/eink-bridge.status 2>/dev/null || echo "(no status yet)"'
**SSH gotcha:** every remote line must be wrapped in `ssh … '…'`. If you paste `/etc/init.d/S99zeink start` on your **laptop**, you get `No such file` — that only exists **on the Pi**.

**Visible refresh gotcha:** `display()` returning only means **SPI bytes were sent**, not that the glass finished. A **960×680 full refresh often needs ~18–25s**. Calling `epd.sleep()` / `module_exit()` immediately **cuts PWR during update** → **no visible change**. `slowos-eink-demo` now **waits `--hold-sec`** (default 18) and does **not** deep-sleep unless you pass **`--deep-sleep`**.

**BUSY GPIO polarity:** if the panel still never moves after a long hold, try **active-low BUSY** (common on SSD16xx):  
`ssh … 'export SLOWOS_EINK_BUSY_INVERT=1; /usr/local/bin/slowos-eink-demo --bars --hold-sec 22'`  
To persist for **`eink-bridge`**, create **`/etc/default/slowos-eink`** on the Pi with `export SLOWOS_EINK_BUSY_INVERT=1` (sourced by **`S99zeink`**).

```bash
ssh -o IdentitiesOnly=yes -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 '/etc/init.d/S99zeink stop; sleep 8; /usr/local/bin/slowos-eink-hw-probe'
ssh -o IdentitiesOnly=yes -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 '/etc/init.d/S99zeink stop; sleep 8; /usr/local/bin/slowos-eink-demo --bars --hold-sec 22'
ssh -o IdentitiesOnly=yes -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 '/etc/init.d/S99zeink stop; sleep 8; /usr/local/bin/slowos-eink-demo --snapshot --display :0 --hold-sec 22'
ssh -o IdentitiesOnly=yes -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 '/etc/init.d/S99zeink start'
```
```
(`eink-bridge` writes **`/run/eink-bridge.status`** JSON on init and each diag interval. **`slowos-eink-hw-probe`**: one-shot SPI + panel init/clear — stop **`S99zeink`** first so SPI is free.)

(`/var/log/eink-init.log`: `S99zeink` / launcher lines when **`:0`** is up but e-ink never starts.)

After SPI exists, validate e-ink while SlowOS stays on **`:0`** (panel-sized root):

```bash
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'DISPLAY=:0 /usr/bin/python3 /usr/bin/eink-bridge --display :0 --no-xvfb'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'DISPLAY=:0 /usr/local/bin/enable-eink'
ssh -i /tmp/pi4_root_key -o StrictHostKeyChecking=no root@192.168.19.138 'DISPLAY=:0 xwd -root -silent >/tmp/hdmi-root.xwd && ls -lh /tmp/hdmi-root.xwd'
```

## Blockers And Failure Modes

- DRM `card0` points at `fec00000.v3d` only (readlink `/sys/class/drm/card0`): Xorg `modesetting` reports `no screens found`; fix KMS/overlay (`vc4-kms-v3d,noaudio`) or rollback overlay before debugging SlowOS apps.
- Missing `/dev/spidev0.0`: e-ink bridge cannot talk to the panel. Fix boot config/device-tree/module exposure first, then power-cycle; do not debug the capture loop until the device exists.
- **Wrong Driver HAT DIP:** Display / Interface must match § **Waveshare E-Paper Driver HAT DIP baseline** above (project assumes **A + 0**). **Interface 1** (3-wire SPI) is **not** implemented in SlowOS.
- **Stale NFS root:** if `/usr/lib/python3/slowos_xwd.py` or current `/usr/bin/eink-bridge` never reached `/nfsroot/rootfs`, `S99zeink` skips — check **`/var/log/eink-init.log`** after rsync.
- `S99zeink` must stay on **`:0`**: it should not export `DISPLAY=:99` or start/kill Xvfb for Phase 2. It mirrors the **primary X root** (panel-first size), not a separate HDMI-only server.
- No window manager: app windows can appear but fail to receive focus quickly or reliably. Validate each app with `xdotool getwindowfocus getwindowname`, especially `slownotes`.
- HDMI unavailable or monitor rejects **960×680**: you lose the **debug mirror**, not the e-ink spine — triage **`/var/log/slowos.log`** (`configure_hdmi_mode`), **`xrandr`**, and **`hdmi_cvt`** / cable / display. Do not revert to **1080p-as-truth** without an explicit product decision.
- NFS-root network disruption: DHCP or interface changes can hang the Pi by dropping the root filesystem network path. Keep the NFS guard intact.

## Rollback Notes

- `S99slowos`: rollback to the previous known-good overlay copy if **`:0`**, panel **`xrandr`** mode (**960×680** / `slowos_*` modeline), input telemetry, or focus readiness regresses. Restart with `/etc/init.d/S99slowos restart` after deploying the rollback.
- `S99zeink`: if e-ink changes break boot or steal the display path, restore disabled-by-default behavior (`SLOWOS_ENABLE_EINK=0`) and remove any `/etc/profile.d/eink.sh` override. After `/etc/init.d/S99slowos restart`, run `/etc/init.d/S99zeink restart` if e-ink was enabled.
- `/etc/network/nfs_check` and `/etc/network/interfaces`: rollback only if non-NFS networking is proven broken; any rollback must preserve NFS-root safety or the Pi can hang during interface bring-up.
- SPI boot/config changes: rollback by removing only the new SPI/device-tree change that hid or destabilized devices, then power-cycle the Pi so firmware and kernel device-tree state are reloaded.

## Done Criteria

- Pi boots to **Xorg `:0`** with **`xdpyinfo` / `xrandr`** showing **960×680** (or the configured **`SLOWOS_PANEL_*`**), keyboard and mouse usable after slowdesktop focus.
- `/dev/spidev0.0` exists.
- **`eink-bridge`** reads **`:0` root** while the **physical HDMI mirror** (when present) shows the **same** logical canvas for fallback/debug.
- Validation commands above pass or produce a captured, actionable failure tied to one blocker in this document.
