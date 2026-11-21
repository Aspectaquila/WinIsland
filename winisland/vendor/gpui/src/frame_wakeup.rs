//! WinIsland patch: let the Windows vsync thread sleep while no window needs a frame.
//!
//! Upstream gpui invalidates every window on every vsync (60–144 wake-ups per second, forever). Windows mark
//! themselves dirty through `WindowInvalidator` / `on_next_frame`; those call `request()`, which wakes the vsync
//! thread. While nothing is requested the thread parks (with a slow timeout for device-lost checks), so an idle
//! app costs ~0% CPU.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::Thread;

static REQUESTED: AtomicBool = AtomicBool::new(true);
static VSYNC: OnceLock<Thread> = OnceLock::new();

/// A window wants to be drawn on the next vsync.
pub(crate) fn request() {
    if !REQUESTED.swap(true, Ordering::SeqCst) {
        if let Some(t) = VSYNC.get() {
            t.unpark();
        }
    }
}

/// Called once from the vsync thread.
#[allow(dead_code)]
pub(crate) fn register_vsync_thread() {
    let _ = VSYNC.set(std::thread::current());
}

/// Take the pending request (vsync thread).
#[allow(dead_code)]
pub(crate) fn take() -> bool {
    REQUESTED.swap(false, Ordering::SeqCst)
}

#[allow(dead_code)]
pub(crate) fn pending() -> bool {
    REQUESTED.load(Ordering::SeqCst)
}
