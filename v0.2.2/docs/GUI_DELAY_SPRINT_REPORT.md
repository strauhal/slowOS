# GUI delay sprint — evidence-tight report (SlowOS / slowdesktop)

Constraints honored: no FS-dominance claim without trace proof; no telemetry impact claim without A/B; OS responsiveness only; **no e‑ink progress claims**.

---

## 1) Live evidence

### A. `SLOWDESKTOP_INPUT_TELEMETRY` on/off (Pi, same script: 8× `xdotool key a`, `/tmp/strace_gx2 -c -f`, ~3 s attach window)

| Metric | `…=0` | `…=1` |
|--------|-------------|-------------|
| `writev` total CPU (strace `seconds` column) | **0.000239** | **0.003259** |
| `writev` calls | 67 | 67 |
| `getdents64` calls | 2 | *(same capture class)* |

**Conclusion:** Telemetry **does** shift **`writev` CPU cost** in this A/B (same syscall count, higher time with tracing on). Not proof of “seconds of wall delay” by itself.

### B. Search-open vs search-closed

**Not run as a paired A/B with identical duration.** Partial capture: **`super+space` + type `doc1`** with **150 dummy files** under `/root/Books/stress` plus short `strace -c`: **`getdents64` = 2**, **`openat` = 4** — **no directory storm** in that slice.

**Light HOME:** Pi `/root/{Books,Music,Documents,Pictures}` entry counts were minimal at time of check.

### C. Populated-tree vs light-tree

Only **stressed** side exercised above (150 files one-off). **No NFS-heavy HOME** reproduction in these captures.

### D. Event → root / window change (“pixel truth”)

| Attempt | Result |
|---------|--------|
| Root `xwd` hash polling | **Pollution:** each `xwd` costs ~tens–hundreds of ms; invalid as latency meter. |
| `xdotool key --window WID` | **Unreliable:** few hash changes per series (focus / delivery). |
| Telemetry **`I`→`P`** same `f=` | **n=90:** **p50 ≈ 6.1 ms**, **p95 ≈ 10.8 ms**, **max ≈ 20 ms** — **inside-frame**, not seconds. |

**Conclusion:** Acceptance metric **C is not satisfied** with current harness; **`I`→`P`** is a partial internal budget only.

---

## 2) Root cause classification

### Proven live offender(s)

- **X11 client path:** `writev` / `recvmsg` / `ppoll` dominate `slowdesktop` syscall profile under scripted keys (`strace -c`).
- **Telemetry on:** higher **`writev` CPU** vs off for matched workload (§1A).

### Probable design hazards (static + fixes applied)

| Hazard | Mitigation in tree |
|--------|---------------------|
| Search app list / PATH probes on GUI thread | `search_app_matches_for_query` cache; `binary_path_cache` + epoch |
| Search file `read_dir` per keystroke | Session **`build_search_file_snapshot`** + in-memory filter |
| Dense selection/hover dither (`rect_filled` loops) | **`draw_dither_selection` / `draw_dither_hover`** → **single translucent `rect_filled`** |
| Continuous repaint at **250 ms** if `RepaintController::new()` + continuous flag | **`RepaintController::with_fast_interval()`**; **`set_continuous`** when **selection / `show_search` / animation** |
| Telemetry default on latency builds | **`S99slowos`:** `SLOWDESKTOP_INPUT_TELEMETRY=${…:-0}` |

### Unresolved

- **Operator-reported ~3 s** on HDMI for **mouse selection + keystrokes** vs **ms-scale `I`→`P`** gap → offender likely **outside that measured window** (input delivery, **Xorg**/present, GPU backlog, or session health) — **not proven here.**

---

## 3) Patch set

| Location | Old | New | Measurement tied to patch |
|----------|-----|-----|---------------------------|
| `slowdesktop/src/desktop.rs` | Per-query FS walk for spotlight files | **`search_file_snapshot`** built once per open-search session | §1B stress slice — **no FS storm** |
| `slowdesktop/src/desktop.rs` | App rows rebuilt every search frame | **`search_app_matches_for_query`** keyed by query + epoch | *(no isolated A/B)* |
| `slowdesktop/src/process_manager.rs` | Repeated PATH resolution | **`binary_path_cache`** | *(no isolated A/B)* |
| `slowcore/src/repaint.rs` | Idle after input unless `needs_repaint` | **`had_input` → `request_repaint_after(16ms)`** | *(no isolated A/B)* |
| `slowcore/src/dither.rs` | Checkerboard loops for selection/hover | **Single translucent overlay rects** | *(no isolated A/B)* |
| `slowdesktop/src/desktop.rs` | `RepaintController::new()` + continuous only on animation | **`with_fast_interval()`** + continuous when **selection / search / animation** | *(no isolated A/B)* |
| `buildroot/rootfs_overlay/etc/init.d/S99slowos` | Telemetry could default on | **Default `:-0`** | §1A |
| `scripts/` | — | `measure-event-to-root.sh`, `strace-count-fs.sh` | §1D caveats |

**Honesty:** Per-patch **before/after latency** was **not** isolated (would require revert binaries); only **telemetry A/B strace** and **stress `strace -c`** are pinned numbers above.

---

## 4) Acceptance

| Item | Status |
|------|--------|
| **p95 event→root-window-change** | **OPEN** — harness invalid / unreliable (§1D). Interim internal gauge: **`I`→`P` p95 ~11 ms** on sampled log lines — **not** acceptance. |
| **Root-buffer / UI truth for e‑ink resume** | **No** — pixel-truth metric **not** validated; **no e‑ink claims** from this sprint. |

---

## Deploy note

Pi **`/usr/bin/slowdesktop`** **must** match workspace **`md5`** after edits (pipe `cat binary \| ssh …` if `scp`/`sftp` unavailable); otherwise fixes never ran on hardware.
