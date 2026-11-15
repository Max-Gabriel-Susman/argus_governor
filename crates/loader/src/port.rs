//! x86 I/O port access.
//!
//! Port I/O is how a PC reaches its legacy devices: COM1 at 0x3F8 and QEMU's
//! isa-debug-exit at 0xF4. Each function is one `in` or `out` instruction.

use core::arch::asm;

/// Reads one byte from `port`.
///
/// # Safety
///
/// Reading a device register can have side effects, such as popping a FIFO or
/// clearing a status bit. The caller must own the device behind `port`.
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: the caller owns the device; `in` touches no memory.
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Writes one byte to `port`.
///
/// # Safety
///
/// Writing a device register reconfigures or triggers the device. The caller
/// must own the device behind `port`.
pub unsafe fn outb(port: u16, value: u8) {
    // SAFETY: the caller owns the device; `out` touches no memory.
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

/// Writes 32 bits to `port`.
///
/// # Safety
///
/// As for [`outb`].
pub unsafe fn outl(port: u16, value: u32) {
    // SAFETY: the caller owns the device; `out` touches no memory.
    unsafe {
        asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags));
    }
}
