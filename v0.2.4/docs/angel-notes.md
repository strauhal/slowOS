# Responses to Angel's Q&A

## A. RAM usage (0.3–0.5 GB)

Confirmed. Each slowOS app is a separate process with its own
eframe/egui/wgpu context and its own copy of the font atlases. With
the desktop shell plus 2–3 open apps we hit that range easily.

Breakdown of where the RAM goes (rough estimate):

| Component                              | Per process  |
|----------------------------------------|--------------|
| eframe + egui + wgpu runtime           | ~40–60 MB    |
| glow / mesa / GL context                | ~30–80 MB   |
| Font atlases (IBM Plex + JetBrains)    | ~5–10 MB     |
| App code + static assets               | ~5–15 MB     |
| Texture caches (cover art, icons, etc.)| variable     |
| X11 / Xorg server (shared)             | ~50 MB       |

The main cost is duplicated GPU/font state across processes. Fixes
ordered by impact:

1. **Strip release binaries** — `strip` + LTO typically cuts 30–50%
   off each binary's VSZ. Already mostly on for release builds.
2. **Share the font atlas via shared memory** — requires egui
   patching. Large effort, big win.
3. **Single-process multi-window architecture** — one eframe
   instance running all apps, each in its own viewport. Biggest
   structural change but biggest RAM win. egui 0.27 supports this
   via `viewport_multiplicity`.
4. **Lazy-load apps** — don't load fonts / textures until the app
   is first opened. Medium effort.
5. **Suspend idle apps** — if an app hasn't been interacted with
   in N minutes, kill it and restore its state on next open. Already
   partly supported via the minimize / restore flow.

For right now, the pragmatic answer is **target the Pi 4** (1 / 2 / 4 GB)
and revisit Pi Zero 2W as a stretch goal once the single-process
architecture work is done.

## B. Keyboard shortcuts / Tab navigation

Full reference: [keyboard.md](./keyboard.md).

Previously `consume_special_keys` stripped Tab globally, which
prevented egui's default focus-cycling. As of today's commit, Tab
now cycles focus in every dialog and app. Shift+Tab cycles
backwards. Enter activates the focused button.

You can now tab through the Open/Save dialogs, the shutdown
confirmation, the USB connect dialog, and so on.

If your Pi isn't picking up the keyboard/mouse at all:

- Check `/proc/bus/input/devices` — are the devices enumerated?
- Check that `/etc/X11/xorg.conf` has `InputClass` sections for
  `evdev keyboard` and `evdev pointer`. The overlay in the repo
  does; if you've overridden xorg.conf, merge those back in.
- `udevadm monitor` while plugging in — does udev see the event?
- If you're running under Wayland + cage, input routing works
  differently. Log as root: `journalctl -b | grep -i libinput`.

## C. Pi Zero 2W won't run as-is

Agreed. Two paths forward:

**Short term: develop on Pi 4.**
The Pi 4 has a real GPU and 1–8 GB RAM. The buildroot target is
ARMv8 Cortex-A53 which works on both the Zero 2W and Pi 4, so the
same image should boot. Just swap the device tree (`bcm2710-*` →
`bcm2711-*`) and increase `gpu_mem`.

**Long term: single-process architecture.**
Fold every app into one binary launched by slowdesktop, each app
running as an egui "viewport" (multi-window within a single
process). This is the only way to hit Zero 2W's RAM budget. Rough
effort: 2–3 weeks of work once we commit.

## D. Design sync after Pi 4 bring-up

Sounds good. Before we meet it'd help if you can put together:

- Candidate e-ink panels (size, resolution, controller chip,
  refresh modes available, driver status — fbtft / vendor SDK /
  custom)
- Power budget (panel + Pi + USB host) and whether a battery is
  in scope
- Whether you're targeting Pi 4 permanently or the Zero 2W is
  still the goal post-optimization
- Whether you've got the kernel building cleanly with `dwc2` and
  `g_mass_storage` enabled (needed for the USB "connect to
  computer" feature)

I'll have:

- USB mass storage flow tested end-to-end on my side (it works in
  the simulator; needs your OTG hardware for real validation)
- Suspend / idle behaviour scoped out (no code yet — we need
  hardware timing first)
- Memory profiling setup so we can actually measure the effect of
  optimizations
