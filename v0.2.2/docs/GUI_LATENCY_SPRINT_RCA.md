# OS-level GUI delay sprint — RCA (evidence-tight)

This document satisfies the sprint handoff: strict separation of **static**, **live-proven**, and **unverified** items; minimal patches; measurement procedures. **No Pi A/B numbers are recorded from this workspace** (no device attached here). Run the listed commands on the Slowbook and paste results into your issue tracker or append below.

---

## 1) Live evidence (fill on device)

### A. Telemetry A/B

| Case | How to run | Event→root (see §C) | Notes |
|------|------------|---------------------|--------|
| Telemetry **on** | Boot with `export SLOWDESKTOP_INPUT_TELEMETRY=1` before `slowdesktop`, or inject via `/etc/profile` / init override | Run `scripts/measure-event-to-root.sh` | Inspect `L,dur_us=` / `P,dur_us=` in `/tmp/slowdesktop-input-telemetry.log` |
| Telemetry **off** | Default in `S99slowos`: `SLOWDESKTOP_INPUT_TELEMETRY=${SLOWDESKTOP_INPUT_TELEMETRY:-0}` | Same | Compare syscall counts from §B |

**Procedure:** same physical reproduction (e.g. select icon, type in spotlight) for both images; collect ≥20 samples per case for p95.

### B. Search-open vs search-closed; populated vs light tree

| Scenario | Procedure |
|----------|-----------|
| Search closed | Reproduce latency without opening Cmd+Space search |
| Search open + typing | Open search; type query that changes each frame |
| Light tree | HOME with few files under Books/Music/… |
| Heavy / NFS tree | Populate Books/slowLibrary + Music with many entries; HOME on NFS if that is production |

**Strace (during lag window):**

```sh
strace -f -tt -T -p "$(pidof slowdesktop)" -o /tmp/slowdesktop.strace
# reproduce, Ctrl+C
./scripts/strace-count-fs.sh /tmp/slowdesktop.strace
```

**Claim discipline:** filesystem churn is **not** proven dominant until `getdents64`/`stat`/`openat` counts **materially rise** vs light-tree baseline **and** correlate in time with the lag window.

### C. Event-to-root-pixel / root buffer change

```sh
cd /path/to/repo/slowos-build/v0.2.2/scripts
DISPLAY=:0 ./measure-event-to-root.sh key a
DISPLAY=:0 ./measure-event-to-root.sh click 400 200
```

Interprets wall time until **root pixmap hash** changes after `xdotool` injects input. Coarse but matches “OS-level truth” for X11 full-frame desktop.

**Record:** `delta_ms` distribution (p50 / p95 / max) per scenario above.

---

## 2) Root cause classification

### 2a) Live-proven (from earlier Pi run cited in sprint; not re-run in CI)

- `slowdesktop` binary matched reviewed build.
- `SLOWDESKTOP_INPUT_TELEMETRY=1` was observed live on a prior image.
- Scripted-window `strace` showed **heavy X11/event-loop syscalls** (`writev`, `recvmsg`, `ppoll`), **not** a `getdents64` storm for that light scenario.

### 2b) Static design hazards (code — proven by inspection)

| Hazard | Location | Risk |
|--------|----------|------|
| Synchronous spotlight **file** scan on GUI thread | `DesktopApp::build_search_file_snapshot` (was per-query `read_dir` churn) | Large HOME / NFS: multi-frame stalls on first non-empty query |
| Synchronous **app** match rebuild | `search_app_matches_for_query` | Was mitigated: cache by `(query, app_state_epoch)` |
| Repeated **PATH/binary** probes | `ProcessManager::binary_path_cache` | Was mitigated: cache per binary name |
| **Per-pixel** selection dither (`density == 1`) | `slowcore::dither::draw_dither_selection` | Thousands of `rect_filled` per frame → CPU/GPU backlog; feels like “seconds” of input lag |
| Telemetry **append** to `/tmp/...` | `BoundedInputTelemetry::flush` | Possible jitter if `/tmp` is slow; magnitude **unproven** without A/B |

### 2c) Still unverified (needs §1 measurements)

- Whether **telemetry on** measurably shifts p95 event→root vs **off** on your image.
- Whether **populated NFS HOME** raises FS syscalls during spotlight typing vs **light tree**.
- Exact split of time among **Rust UI**, **egui tessellation**, **Mesa/GL**, and **Xorg** (`perf` on device).

---

## 3) Patch set (this sprint)

| File / symbol | Old behavior | New behavior |
|----------------|--------------|--------------|
| `slowcore/src/dither.rs` — `draw_dither_selection` | `draw_dither_rect(..., density: 1)` — one tiny rect per checker cell | `density: 2` — coarser checkerboard, **far fewer** draw calls |
| `slowdesktop/src/desktop.rs` — spotlight files | Per-**query** `read_dir` + filter (new query → new FS walk) | **One** `build_search_file_snapshot()` per search session (first non-empty query); `filter_file_snapshot_for_query` in RAM; cap `SEARCH_FILE_SNAPSHOT_CAP` (500) |
| `slowdesktop/src/desktop.rs` — `search_app_matches_for_query` | (Prior) rebuild every frame while search open | Cache invalidates on `(query, process_manager.app_state_epoch())` |
| `slowdesktop/src/process_manager.rs` — `binary_path_cache` | (Prior) repeated PATH resolution | Cache `Option<PathBuf>` per binary; epoch bump when `running` changes |
| `buildroot/rootfs_overlay/etc/init.d/S99slowos` | — | `export SLOWDESKTOP_INPUT_TELEMETRY="${SLOWDESKTOP_INPUT_TELEMETRY:-0}"` — **default off** for latency builds |
| `scripts/measure-event-to-root.sh` | — | Injects key/click; polls root `xwd` hash until change |
| `scripts/strace-count-fs.sh` | — | Offline FS-related syscall counts from strace log |

**Per-patch before/after:** capture §1C and §1B on the **same** hardware image, swapping only the single variable (telemetry, or binary build) between runs.

---

## 4) Acceptance

- **Metric:** p95 **event→root-window-change** (`scripts/measure-event-to-root.sh`), search-closed baseline vs search-open stress vs telemetry A/B.
- **Proposed target (set after baseline):** e.g. **p95 ≤ 200 ms** for search-closed desktop interactions on HDMI — **revise** once you have real numbers; do not treat as signed-off here.
- **E-ink:** No resume of e-ink integration claims until the above metric is stable enough for your product bar; this sprint scope is **OS responsiveness only**.

---

## 5) Operator checklist (Pi)

1. Deploy rebuilt `slowdesktop` + `slowcore` (release).
2. Confirm init: `grep SLOWDESKTOP_INPUT_TELEMETRY /etc/init.d/S99slowos` → default `0` unless overridden.
3. Run telemetry **off** session: 20× `measure-event-to-root.sh` for click + key.
4. Force telemetry **on** (export in init), repeat.
5. Optional: strace + `strace-count-fs.sh` for search-heavy typing on NFS HOME.
