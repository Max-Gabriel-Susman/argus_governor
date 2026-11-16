//! A raw Embassy executor and the loop that polls it.
//!
//! `embassy-executor` is built without a `platform-*` feature, so it brings
//! no run loop of its own. It needs two things from us:
//!
//! - a function exported as `__pender`, which it calls whenever a task is
//!   spawned or woken. Ours only raises a flag;
//! - something that calls `poll` after that. Here that's [`run`].
//!
//! No interrupts are set up yet, so nothing could wake the CPU from `hlt`.
//! The loop spins on the flag instead.

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::{Spawner, raw};
use static_cell::StaticCell;

/// Raised by the pender, lowered by [`run`] just before it polls.
static WORK_PENDING: AtomicBool = AtomicBool::new(false);

/// `poll` and `spawner` need a `&'static` executor. A `StaticCell` hands out
/// that reference once, at run time.
static EXECUTOR: StaticCell<raw::Executor> = StaticCell::new();

/// Called by the executor when a task becomes ready to run.
///
/// It must not call `poll` itself, because `poll` can call the pender and
/// `poll` must never run reentrantly. So it only records that there is work.
#[unsafe(export_name = "__pender")]
fn pender(_context: *mut ()) {
    WORK_PENDING.store(true, Ordering::Release);
}

/// Creates the executor, lets `spawn` start tasks on it, then polls it until
/// `done` returns true.
pub fn run(spawn: impl FnOnce(Spawner), done: impl Fn() -> bool) {
    // The context pointer is handed back to the pender. One executor needs
    // none. EXECUTOR.init panics if run is ever called twice.
    let executor: &'static raw::Executor = EXECUTOR.init(raw::Executor::new(core::ptr::null_mut()));
    spawn(executor.spawner());

    while !done() {
        if WORK_PENDING.swap(false, Ordering::AcqRel) {
            // SAFETY: only this loop calls poll, and the pender never does, so
            // poll is not reentered.
            unsafe { executor.poll() };
        } else {
            core::hint::spin_loop();
        }
    }
}
