//! governor_loader: the UEFI stage of the argus_governor edge node.
//!
//! Milestone 1: greet through the firmware, once on the UEFI console (ConOut)
//! and once on the serial port (the Serial I/O protocol), then power off so
//! the run ends by itself.

#![no_std]
#![no_main]

mod serial;

use core::fmt::Write;

use uefi::prelude::*;
use uefi::runtime::ResetType;

use crate::serial::SerialWriter;
use crate::serial::firmware::FirmwareSerial;

const GREETING: &str = "Hello, Atom! (UEFI)";

#[entry]
fn main() -> Status {
    // 1. The UEFI console. OVMF mirrors ConOut onto COM1, so this line also
    //    reaches the terminal, wrapped in the firmware's terminal escapes.
    uefi::println!("{GREETING}");

    // 2. The serial port, through the firmware's Serial I/O protocol.
    match FirmwareSerial::open() {
        Ok(mut port) => {
            // SerialWriter never fails, so there is no error to handle.
            let _ = writeln!(SerialWriter(&mut port), "{GREETING}");
        }
        Err(err) => uefi::println!("no serial port: {err}"),
    }

    // 3. Power off. QEMU exits with status 0; a real board turns off.
    runtime::reset(ResetType::SHUTDOWN, Status::SUCCESS, None)
}
