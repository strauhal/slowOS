//! Partial repaint controller for slowOS v0.2.2
//!
//! egui is an immediate-mode GUI: every frame redraws everything.  On an
//! e-ink display this is expensive — each refresh is visible and slow.
//!
//! `RepaintController` sits between the app and egui's repaint scheduler.
//! It tracks *why* a repaint is needed and suppresses unnecessary ones:
//!
//! 1. **Input-driven** — user typed, clicked, or scrolled.  Always repaint.
//! 2. **Timed** — an animation or clock tick fired.  Repaint at a governed
//!    rate (default 250 ms, ~4 Hz for e-ink).
//! 3. **Idle** — nothing happened.  Do *not* repaint.
//!
//! Apps call `rc.mark_needs_repaint()` when internal state changes outside
//! of an input event (e.g. a timer fires, playback advances).  The
//! controller coalesces these into at most one repaint per interval.
//!
//! For apps that need continuous repainting (slowMidi playback, slowBreath
//! animation), call `rc.set_continuous(true)` to keep the repaint timer
//! running.  Call `rc.set_continuous(false)` when the activity stops.

use std::time::{Duration, Instant};

/// Default repaint interval for timed updates (e-ink friendly ~4 Hz).
const DEFAULT_REPAINT_INTERVAL: Duration = Duration::from_millis(250);

/// Repaint interval for apps that explicitly need faster updates.
const FAST_REPAINT_INTERVAL: Duration = Duration::from_millis(33);

/// Why this frame is being painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepaintReason {
    /// First frame — always paint.
    Init,
    /// User input (mouse move, key press, scroll).
    Input,
    /// App-requested repaint (state changed internally).
    StateChange,
    /// Timed continuous repaint (animation, playback).
    Continuous,
}

/// Controls when the egui context should request repaints.
///
/// Drop this into your app struct and call [`begin_frame`] at the top of
/// `update()` and [`end_frame`] at the bottom.
pub struct RepaintController {
    /// Whether continuous (timed) repainting is active.
    continuous: bool,
    /// Whether a one-shot repaint has been requested.
    needs_repaint: bool,
    /// Repaint interval when continuous is active.
    interval: Duration,
    /// Last time a repaint was actually issued.
    last_repaint: Instant,
    /// Frame counter (0 = first frame).
    frame: u64,
    /// Why the current frame is being painted (set by begin_frame).
    reason: RepaintReason,
    /// Whether any input events were present this frame.
    had_input: bool,
}

impl Default for RepaintController {
    fn default() -> Self {
        Self::new()
    }
}

impl RepaintController {
    pub fn new() -> Self {
        Self {
            continuous: false,
            needs_repaint: false,
            interval: DEFAULT_REPAINT_INTERVAL,
            last_repaint: Instant::now(),
            frame: 0,
            reason: RepaintReason::Init,
            had_input: false,
        }
    }

    /// Create a controller that uses a faster repaint interval.
    /// Use this for apps that need smoother animation during their
    /// continuous phase (e.g. slowMidi at 30 fps during playback).
    pub fn with_fast_interval() -> Self {
        Self {
            interval: FAST_REPAINT_INTERVAL,
            ..Self::new()
        }
    }

    /// Enable or disable continuous (timed) repainting.
    ///
    /// When `true`, the controller will schedule repaints at its configured
    /// interval until `set_continuous(false)` is called.
    ///
    /// Use this for:
    /// - Active playback (slowMidi, slowMusic)
    /// - Running animations (slowBreath circle, slowClock stopwatch)
    /// - Any time-driven display update
    pub fn set_continuous(&mut self, continuous: bool) {
        self.continuous = continuous;
    }

    /// Returns whether continuous mode is active.
    pub fn is_continuous(&self) -> bool {
        self.continuous
    }

    /// Request a single repaint on the next opportunity.
    ///
    /// Call this when internal state changes outside of user input — for
    /// example, when a background thread completes loading, or a timer
    /// transitions to a new phase.
    pub fn mark_needs_repaint(&mut self) {
        self.needs_repaint = true;
    }

    /// Returns why the current frame is being painted.
    pub fn reason(&self) -> RepaintReason {
        self.reason
    }

    /// Current frame counter.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Call at the **start** of your `update()` method.
    ///
    /// Inspects the egui input to determine why this frame is running
    /// and sets the repaint reason accordingly.
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        self.had_input = ctx.input(|i| {
            // Any mouse movement, button press, scroll, or key event counts
            !i.events.is_empty()
                || i.pointer.any_pressed()
                || i.pointer.any_released()
                || i.pointer.any_click()
                || i.raw_scroll_delta != egui::Vec2::ZERO
                || i.pointer.is_moving()
        });

        self.reason = if self.frame == 0 {
            RepaintReason::Init
        } else if self.had_input {
            RepaintReason::Input
        } else if self.needs_repaint {
            RepaintReason::StateChange
        } else if self.continuous {
            RepaintReason::Continuous
        } else {
            // Shouldn't normally get here (frame was triggered by
            // something), but treat it as input-driven to be safe.
            RepaintReason::Input
        };

        // Clear the one-shot flag now that we've consumed it.
        self.needs_repaint = false;
    }

    /// Call at the **end** of your `update()` method.
    ///
    /// Schedules the next repaint if needed:
    /// - Continuous mode → repaint after the configured interval.
    /// - One-shot request pending → immediate repaint.
    /// - Otherwise → no repaint (egui will wake on next input event).
    pub fn end_frame(&mut self, ctx: &egui::Context) {
        self.frame += 1;

        if self.continuous {
            ctx.request_repaint_after(self.interval);
            self.last_repaint = Instant::now();
        } else if self.needs_repaint {
            // Something was marked dirty during this frame's UI code.
            ctx.request_repaint();
            self.last_repaint = Instant::now();
        }
        // else: no scheduled repaint — egui sleeps until next input.
    }
}
