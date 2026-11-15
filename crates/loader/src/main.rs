//! governor_loader: the UEFI stage of the argus_governor edge node.
//!
//! Milestone 1: greet through the firmware, once on the UEFI console (ConOut)
//! and once on the serial port (the Serial I/O protocol).
//! Milestone 2: exit boot services, greet again by driving the COM1 UART
//! directly, and end the QEMU run through isa-debug-exit.

#![no_std]
#![no_main]

mod panic;
mod port;
mod qemu;
mod serial;

use core::arch::asm;
use core::fmt::Write;

use uefi::prelude::*;
use uefi::runtime::ResetType;

use crate::qemu::ExitCode;
use crate::serial::SerialWriter;
use crate::serial::firmware::FirmwareSerial;
use crate::serial::uart16550::{BAUD_115200, COM1_BASE, PortIo, Uart16550};

const GREETING: &str = "Hello, Atom! (UEFI)";

#[entry]
fn main() -> Status {
    // 1. The UEFI console. OVMF mirrors ConOut onto COM1, so this line also
    //    reaches the terminal, wrapped in the firmware's terminal escapes.
    uefi::println!("{GREETING}");

    // 2. The serial port, through the firmware's Serial I/O protocol. The
    //    protocol is closed at the end of the Ok arm, while boot services
    //    still exist to close it.
    match FirmwareSerial::open() {
        Ok(mut port) => {
            // SerialWriter never fails, so there is no error to handle.
            let _ = writeln!(SerialWriter(&mut port), "{GREETING}");
        }
        Err(err) => uefi::println!("no serial port: {err}"),
    }

    // 3. Leave the firmware. The memory map is what a kernel needs next; for
    //    now it is only kept.
    //
    // SAFETY: no protocol is open, nothing allocates, and nothing below uses
    // boot services. In particular, no more uefi::println!.
    let _memory_map = unsafe { boot::exit_boot_services(None) };

    // The IDT still points at firmware handlers, in memory that is now ours.
    // EDK2-based firmware masks interrupts in ExitBootServices already; this
    // makes it explicit.
    //
    // SAFETY: clearing IF only blocks maskable interrupts.
    unsafe { asm!("cli", options(nomem, nostack)) };

    // 4. COM1, driven directly: a 16550 at I/O port 0x3F8, 115200 8N1.
    //
    // SAFETY: the firmware's serial driver stopped with boot services. The
    // only other user of COM1 is the panic handler, which takes it over only
    // when this code can no longer run.
    let regs = unsafe { PortIo::new(COM1_BASE) };
    let mut com1 = Uart16550::new(regs, BAUD_115200);
    let _ = writeln!(
        SerialWriter(&mut com1),
        "Hello from bare metal: boot services exited"
    );

    // 5. Report success to the run script.
    qemu::exit(ExitCode::Success);

    // Only reached without isa-debug-exit. Runtime services survive
    // ExitBootServices, so the firmware can still power the machine off.
    runtime::reset(ResetType::SHUTDOWN, Status::SUCCESS, None)
}
