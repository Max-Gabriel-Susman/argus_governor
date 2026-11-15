//! Ending a QEMU run with a status the run script can check.
//!
//! The run script adds QEMU's isa-debug-exit device at I/O port 0xF4. When the
//! guest writes `value` there, QEMU exits with status `(value << 1) | 1`, so
//! the two codes below become exit statuses 33 and 35. The script turns 33
//! into 0 and everything else into a failure.

use crate::port;

/// I/O port of the isa-debug-exit device (`iobase=0xf4` in the run script).
const ISA_DEBUG_EXIT: u16 = 0xF4;

/// What to report to the run script.
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum ExitCode {
    /// QEMU exits with 33.
    Success = 0x10,
    /// QEMU exits with 35.
    Failure = 0x11,
}

/// Ends the QEMU run with `code`.
///
/// Returns only when there is no isa-debug-exit device, which is why callers
/// follow it with their own way of stopping.
pub fn exit(code: ExitCode) {
    // SAFETY: this loader only runs under QEMU so far, where the run script
    // puts isa-debug-exit on this port. Board bring-up replaces this call.
    unsafe { port::outl(ISA_DEBUG_EXIT, code as u32) };
}
