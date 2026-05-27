# Pi 4 Phase 2.5 — E-Ink QoL, responsiveness, anti-stale
> **Note:** This document builds on the parent (see top of `PI4_PHASE2_EINK_HDMI_HANDOFF.md`).
> References to gx2 paths or NFS are development-environment examples only.
> See the parent document for guidance on adapting to other setups.

**Parent contract:** Phase 2 display and boot spine stay authoritative — see **`PI4_PHASE2_EINK_HDMI_HANDOFF.md`** (panel-first **960×680**, **`DISPLAY=:0`** capture, HDMI mirror). Phase 2.5 **does not** redefine that spine; it tightens **`eink-bridge`** behavior and related policy so e-ink feels first-class next to HDMI.

## Mission

- **E-ink remains primary; HDMI is mirror/debug** (unchanged product stance from Phase 2).
- **Quality of life:** drastic improvement in perceived responsiveness for **pointer** and **interactive edits** (typing, focus, selection), within SSD16xx physics.
- **Non-negotiable:** If HDMI clearly shows cursor motion or material UI change, e-ink must **not** sit unchanged indefinitely while the bridge “believes” a no-op (no silent infinite stall vs HDMI).
- **Quality order for this phase:** responsiveness and **correctness vs HDMI** over minimal ghosting. **Ghosting is acceptable** until a later polish milestone; optimize for **fast partials** first.

## Locked product decisions (do not renegotiate in implementation)

| Decision | Choice |
|----------|--------|
| **Update rate target** | Panel path targets **up to 6 Hz** end-to-end **attempt** rate where CPU, capture, SPI, and BUSY allow (document measured ceiling on reference Pi + HAT). |
| **Escalation shape** | **From first failure:** deadline-based ladder (not periodic full refreshes “every 6s” while healthy). Avoid unnecessary load when healthy. |
| **Recovery scope** | On persistent degraded state after full-panel recovery attempts: **restart `eink-bridge` process only** — **no** `S99slowos` / init / machine reboot in Phase 2.5 (isolates failures for debug and test runs). |

## Non-goals

- **No IT8951 path** and no “drop in PaperTTY / GregDMeyer IT8951 firmware.” Production remains **SSD16xx / `epd13in3k`** only; borrow **algorithms** (cursor-led bbox, merge, reconcile), not foreign silicon commands.
- **No Rust e-ink rewrite** and no mandatory Wayland/cage migration for 2.5.
- **No redefining ~3s desktop/GTK latency** as solely an e-ink bug unless instrumentation proves capture/bridge is the dominant contributor; Phase 2.5 focuses on **bridge + pointer policy**.

## Hardware and code path (in scope)

- **Waveshare 13.3" K** stack: **`waveshare_epd` / `epd13in3k.py`** (SSD16xx-class), **`eink-bridge`** on **`DISPLAY=:0`**, existing cursor overlay (`xdotool` + diff) unless replaced with an equivalent **documented** mechanism.
- Touch **`slowdesktop` / Xorg init** only when a bridge change **requires** a small hook (e.g. env, status path); default is **bridge-first**.

## Cadence and streams (reconciles 6 Hz pointer vs 3s reconcile)

Implement **separate logical streams** so a global slow timer cannot starve the cursor.

| Stream | Role | Rule |
|--------|------|------|
| **Pointer-led** | Small bbox following cursor (PaperTTY-style “tile around input”). | **High priority**; **target up to 6 Hz** for this path. **Must not** be blocked by the 3s reconcile below. |
| **Heavy root / tile diff** | `xwd` + PIL diff, tiles, MD5-style gates, multi-rect merge. | Batches/coexists with pointer stream; alignment and merge rules stay SSD16xx-safe. |
| **Anti-stale reconcile** | Guarantees the bridge cannot forever no-op while HDMI diverged. | **At least every 3s:** a reconciliation pass that **cannot be skipped** by hash/cache heuristics alone (full frame compare path, wide partial, or other defined “truth vs `_last_push_l`” audit — implementor picks minimal SPI cost that restores correctness). |

**Keystroke / selection parity:** E-ink should be **no worse than** current observed **~3s P95** for root content updates **once X has painted** (same bar as Phase 2 operational reality unless a separate desktop task lowers it).

## Multi-region policy (PaperTTY-inspired, dual track)

- **Track A — Caret / typing / focused field:** Stable or slowly moving bbox; **merge** small vertical/horizontal edits to limit SPI partial count.
- **Track B — Mouse:** Cursor-following bbox; **wins scheduling** when union exceeds per-frame region cap.
- **Per frame cap:** **N** regions (implementor: **2–4**, document default). Overflow policy: **pointer first**, then caret track, then largest dirty tile from root diff.

## Failure and escalation ladder (from first failure)

Use **deadlines from first detected push failure** (or entry into **degraded** cache confidence — define in code/status contract):

1. **T+0:** failure observed; do not treat belief as updated.
2. **By T+3s from first failure:** **attempt** full-panel recovery refresh (waveform per existing escalation).
3. **By T+6s from first failure:** if still failing, **hard** full-panel attempt (or escalate to strongest defined refresh in driver) per existing `eink-bridge` semantics.
4. **If degraded persists** after full-panel ladder: **restart `eink-bridge` process only** (supervisor/init script may wrap restart; **no full machine reboot** in 2.5).

Tune internal constants in code; **document** env vars for operators (e.g. reconcile interval, Hz cap, T+3/T+6).

## Success criteria (acceptance / QA)

- **Mouse:** Slow continuous motion on HDMI → e-ink cursor blob updates at **up to ~6 Hz** (subject to measured ceiling); **no** multi-second freeze while HDMI cursor moves (unless documented saturation — then status must reflect backpressure).
- **Anti-stale:** Scenarios where only root pixmap changes → e-ink **updates within ≤3s** or shows **explicit degraded state** in **`/run/eink-bridge.status`** (optional but preferred over silent failure).
- **Escalation:** Injected repeated push failures trigger **deadline-based full panel** then **process restart** without rebooting the Pi.

## Deliverables (orchestrator)

1. **Code + tests** in **`eink-bridge`** and, only if required, **`waveshare_epd/epd13in3k.py`** / shared helpers — within scope above.
2. **Documented environment variables** for: max attempt Hz (default toward **6**), **3s** reconcile interval, **T+3 / T+6** escalation, per-frame region cap, feature flags for dual-track behavior.
3. **One-command or short documented verify** from clean gx2 checkout (or Pi-side log + status checks) so reviewers can confirm behavior without ad-hoc steps.

## Implementation plan (orchestrator seed)

Seeded from codebase-planner subagent against current tree; execute in order, adjust line anchors if `eink-bridge` shifts.

1. **Baseline the current bridge contract** — Read `buildroot/rootfs_overlay/usr/bin/eink-bridge` (`run()` main loop ~741-809: `capture_screen` then full-frame **MD5** vs `_last_pushed_hash` then coalesced `push_frame`; `_last_push_l` in `push_frame` for dirty partials). Document the gap vs Phase 2.5 (single timer path can starve pointer or skip reconcile when hash-stable).

2. **Define stream scheduling in `push_frame` / `run` (same file)** — Split **pointer-led** (small cursor bbox, high priority, target ~167 ms between attempts when motion present for ~6 Hz), **heavy root** (`xwd` + PIL diff / existing tile merge), and **anti-stale reconcile** (wall-clock <= 3 s cadence; **cannot** be skipped solely because `frame_hash == _last_pushed_hash`). Preserve SSD16xx alignment and `_last_push_l` paste contract from the module docstring. Rollback via feature envs in step 3.

3. **Operator-tunable environment variables** (read in bridge `__init__` / argparse; document in this file header comment block on `eink-bridge`):
   - `SLOWOS_EINK_RECONCILE_INTERVAL_SEC` — max wall time between reconcile passes (default **3**).
   - `SLOWOS_EINK_POINTER_MAX_HZ` (or min-interval) — cap pointer-led attempts (default **6**; align with `SLOWOS_EINK_FPS` / launcher `--fps`).
   - `SLOWOS_EINK_ESCALATE_FULL_SEC`, `SLOWOS_EINK_ESCALATE_HARD_SEC` — from first push failure (**3** / **6** s).
   - `SLOWOS_EINK_DUAL_TRACK` — caret vs mouse dual-track (default **on** when implemented; **off** = Phase 2-like path).
   - `SLOWOS_EINK_DISABLE_RECONCILE` / `SLOWOS_EINK_DISABLE_POINTER_STREAM` — emergency rollback.
   Reuse or tune **`SLOWOS_EINK_DIRTY_MAX_REGIONS`** (handoff: **2-4** per frame; lower from **6** if spec requires).

4. **Per-frame region cap and overflow policy** — Pointer first, then caret/slow-edit track, then largest dirty tile when merged rects exceed cap. Keep `SLOWOS_EINK_DIRTY_MERGE_GAP_PX`, `SLOWOS_EINK_DIRTY_TILE`, `SLOWOS_EINK_DIRTY_AREA_MAX` compatible with Phase 2 table in `PI4_PHASE2_EINK_HDMI_HANDOFF.md`. Verify `dirty_rect_count_last` / `regions_last` in logs and status under load.

5. **Extend `/run/eink-bridge.status` JSON** (`_status_payload`) — stream/backpressure hints, `last_reconcile_mono` (or wall time), explicit degraded when anti-stale cannot run. Verify with `cat /run/eink-bridge.status` on Pi.

6. **Failure ladder + process-only recovery** — From first push failure: **T0**; by **T+3s** attempt full-panel recovery; by **T+6s** strongest defined refresh; if still degraded, **exit non-zero** so **`/etc/init.d/S99zeink restart`** restarts only `eink-bridge` (no `S99slowos` reboot). Optional: minimal watchdog loop in `buildroot/rootfs_overlay/usr/local/sbin/slowos-eink-launch.sh` after BusyBox constraints check.

7. **Driver** — Touch `epd13in3k.py` only if hardest refresh is not already exposed from bridge.

8. **Persistent overrides** — Document exports in `/etc/default/slowos-eink` (sourced by `S99zeink`); prefer env-only in Python over new CLI unless necessary.

9. **Reviewer verification** — Add one-command or <=3-step gx2 path (e.g. `scripts/pi4-eink-prove-from-gx2.sh` where applicable) plus Pi: `tail -f /var/log/eink-bridge.log`, `cat /run/eink-bridge.status`, `DISPLAY=:0 xdpyinfo` for **960x680**.

10. **Acceptance QA on hardware** — Mouse ~6 Hz; anti-stale <=3 s; escalation ladder then `S99zeink restart` without full reboot. Cross-check `SLOWOS_EINK_DIRTY_RECT`, `SLOWOS_EINK_CURSOR_OVERLAY`, `SLOWOS_EINK_FIT` defaults per Phase 2 doc.

**Rollback:** `SLOWOS_EINK_DUAL_TRACK=0` and/or `SLOWOS_EINK_DISABLE_RECONCILE=1`; operators use `/etc/init.d/S99zeink restart`.

**Preconditions:** NFS vs overlay path drift — fix deploy first (`/var/log/eink-init.log`). BUSY anomalies — `SLOWOS_EINK_BUSY_INVERT` per Phase 2 doc before timing proofs.

## Operator env (Phase 2.5)

| Variable | Default | Role |
|----------|---------|------|
| `SLOWOS_EINK_PHASE25` | `1` | Master gate (`0` = legacy single-stream loop). |
| `SLOWOS_EINK_POINTER_HZ` | `6` | Pointer-led partial attempt cap; bypasses main `SLOWOS_EINK_MIN_PUSH_INTERVAL_SEC` coalesce. |
| `SLOWOS_EINK_RECONCILE_SEC` | `3` | Anti-stale displayed-model vs capture audit interval (cannot be skipped by hash-only shortcuts). |
| `SLOWOS_EINK_ESCAL_SOFT_SEC` | `3` | From first push failure: force full-panel partial by this deadline. |
| `SLOWOS_EINK_ESCAL_HARD_SEC` | `6` | From first push failure: force full mono refresh path by this deadline. |
| `SLOWOS_EINK_REGION_CAP` | `3` | Dual-track max regions per partial (2–4; pointer wins on overflow). |
| `SLOWOS_EINK_POINTER_ARENA` | `96` | Cursor-following bbox characteristic size (px). |
| `SLOWOS_EINK_RESTART_GRACE_SEC` | `2` | After hard escal deadline, consecutive failures before `exec` process restart (no reboot). |
| `SLOWOS_EINK_WATCHDOG_FRAMES` | `24` | After N captures with no successful push, byte-compare `_last_push_l` vs capture; if different, force `watchdog` push (breaks MD5 / stale-belief deadlocks). |
| `SLOWOS_EINK_FOCUS_HINTS` | `1` | Poll `xdotool` active window id/name/geometry; one-shot `focus` push on change + merge hint into reconcile/hash/watchdog. |
| `SLOWOS_EINK_FOCUS_POLL_HZ` | `4` | Active-window poll rate. |
| `SLOWOS_EINK_POINTER_TRAIL` | `1` | Union previous+current pointer arenas in partial rects (ghost reduction). |

**Pi verify (SSH):** `cat /run/eink-bridge.status` — expect `phase25`, `pointer_hz`, `last_reconcile_wall` / `last_reconcile_mono`, `escalation_deadline_mono` when failing, and `degraded_reason` when applicable; **`push_reason_last`**, **`captures_since_push`**, **`audit_due`**, **`hash_same_count`**, **`caret_path_used`**, **`focused_window_id`**, **`region_merge_mode`** for scheduler diagnosis; `tail -f /var/log/eink-bridge.log` for `phase25 push` debug lines.

**Phase 2.5 producer/consumer (when `phase25` is true):** a capture thread fills a **depth-1** latest-frame slot while the main thread alone runs diff, scheduling, SPI, and **`_last_push_l`** updates. Status includes **`producer_consumer`**, **`capture_seq`**, **`push_seq`**, **`producer_overwrites`**, **`latest_frame_age_ms`**, **`last_capture_to_push_ms`**, **`last_loop_ms`**, **`captures_per_wall_s`**, **`pushes_per_wall_s`**, **`last_no_push_reason`**, **`last_regions_premerge`** / **`last_regions_postmerge`**, overlap union audit fields, and **`last_dirty_area_px_raw`** / **`last_dirty_area_px_effective`** (per consumer pass).

## § Powercycle + deploy

Follow **`PI4_PHASE2_EINK_HDMI_HANDOFF.md`** **Deterministic boot procedure** (order matters; skipping steps causes HDMI-only or stale-root symptoms).

1. On gx2: `git pull` this repo — overlay source of truth is **`buildroot/rootfs_overlay/`**.
2. **NFS rootfs:** from `slowos-build/v0.2.2/scripts` run **`sudo ./rsync-nfs-rootfs-overlay.sh`** (set **`SLOWOS_NFS_ROOT`** if your export path is not the default). This publishes `eink-bridge`, `S99zeink`, Python helpers, and fixes **`/root/.ssh`** perms for Dropbear.
3. If **`config.txt`**, kernel **`Image`**, or TFTP boot assets changed, also run **`sudo ./sync-pi4-tftp-videocore-from-buildroot.sh`** or **`SLOWOS_SYNC_TFTP=1 sudo ./deploy-pi4-phase2-deterministic.sh`** per Phase 2 doc.
4. **Cold power-cycle** the Pi (firmware + DT + clean NFS mount state).
5. If the Pi still runs an old bridge, read **`/var/log/eink-init.log`** first (Phase 2: stale NFS / missing overlay files).

Persistent Phase 2.5 toggles ship as **`buildroot/rootfs_overlay/etc/default/slowos-eink`** (all lines commented; **`S99zeink`** sources it). Uncomment **`export …`** lines on the Pi only when you need non-default behavior (e.g. **`SLOWOS_EINK_BUSY_INVERT=1`**).

## § User acceptance test (Phase 2.5)

After deploy + power-cycle, on the Pi over SSH (HDMI remains mirror of **`DISPLAY=:0`**, **960×680** spine unchanged):

1. **Boot / display:** Confirm Xorg **`DISPLAY=:0`** at panel resolution (`xdpyinfo` / `xrandr` as in Phase 2 doc).
2. **Logs:** `tail -f /var/log/eink-bridge.log` (optional: `tail -f /var/log/eink-init.log` while the background launcher waits for SPI/X).
3. **Status:** `cat /run/eink-bridge.status` — JSON should include **`phase25`** (true unless you set **`SLOWOS_EINK_PHASE25=0`**), **`pointer_hz`**, **`reconcile_sec`**, **`last_reconcile_wall`** / **`last_reconcile_mono`** updating over time, and **`degraded_reason`** when the bridge is unhappy.
4. **Pointer:** Move the mouse slowly on HDMI; the e-ink cursor track should update regularly (target up to ~6 Hz, subject to BUSY/SPI), not freeze for multi-second gaps while HDMI moves.
5. **Typing / app:** In a focused app, type and edit; e-ink should track HDMI within the **≤~3 s** anti-stale window for root content (after X has painted), not sit unchanged indefinitely while HDMI shows edits.
6. **Stale vs HDMI:** If HDMI shows a clear UI change and e-ink does not catch up, status/logs should show **degraded** or push errors — not silent “healthy.” Recovery: **`/etc/init.d/S99zeink restart`** (process-only; no full machine reboot required for Phase 2.5).

## Key paths (SlowOS v0.2.2)

- **`buildroot/rootfs_overlay/usr/bin/eink-bridge`**
- **`buildroot/rootfs_overlay/usr/lib/python3/waveshare_epd/epd13in3k.py`**
- Status: **`/run/eink-bridge.status`** (extend fields as needed for streams, degraded reason, last reconcile time).
- Init: **`buildroot/rootfs_overlay/etc/init.d/S99zeink`** (process restart only; do not expand to system reboot).
- Defaults template: **`buildroot/rootfs_overlay/etc/default/slowos-eink`** (comment-only; sourced at boot).
- Launcher (watchdog / `--fps`): **`buildroot/rootfs_overlay/usr/local/sbin/slowos-eink-launch.sh`**

## Assumptions

- **6 Hz** is a **target**, not a hard real-time guarantee — BUSY and full-frame capture dominate worst cases; implementors **measure** and document.
- Pointer fidelity remains bounded by **`xwd` root capture** and overlay latency until a future architecture (damage regions, etc.) is explicitly scoped.

## Rollback

- Ship flags to **disable** dual-track or new reconcile timer if regressions appear; default Phase 2 behavior should remain reachable via **`SLOWOS_*`** toggles documented in this repo’s README or overlay comments.
- Process-only restart policy: operators can **`/etc/init.d/S99zeink restart`** (or equivalent) without cycling the full system.
