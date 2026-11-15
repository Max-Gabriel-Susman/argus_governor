//! The panic handler.
//!
//! It works with or without boot services, because it drives COM1 directly
//! instead of going through the firmware. It prints the panic, then ends the
//! QEMU run with the failure code, so a panic shows up in the serial log and
//! as a failed `cargo run`.

use core::fmt::Write;
use core::panic::PanicInfo;

use crate::qemu::{self, ExitCode};
use crate::serial::SerialWriter;
use crate::serial::uart16550::{BAUD_115200, COM1_BASE, PortIo, Uart16550};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // SAFETY: whatever else was using COM1 won't run again after a panic, so
    // taking the port over is safe, and it's the only way left to report.
    let regs = unsafe { PortIo::new(COM1_BASE) };
    let mut com1 = Uart16550::new(regs, BAUD_115200);
    let _ = writeln!(SerialWriter(&mut com1), "\npanic: {info}");

    qemu::exit(ExitCode::Failure);

    // No isa-debug-exit: stop here.
    loop {
        core::hint::spin_loop();
    }
}
