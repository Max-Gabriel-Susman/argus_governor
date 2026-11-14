//! Serial output behind one small trait.
//!
//! Everything that prints over serial goes through [`SerialPort`], so the
//! backend can change without touching the code that prints:
//!
//! - [`firmware::FirmwareSerial`]: the firmware's Serial I/O protocol. Only
//!   valid while boot services are running.
//! - Next: a 16550 UART driven directly through I/O ports (COM1 at 0x3F8),
//!   and later the same UART behind memory-mapped registers if the board's
//!   header turns out to be wired that way.

pub mod firmware;

use core::fmt;

/// A byte sink that ends at a UART.
pub trait SerialPort {
    /// Sends `bytes` exactly as given, blocking until every byte is accepted.
    fn write_bytes(&mut self, bytes: &[u8]);
}

/// Lets `write!` and `writeln!` target any [`SerialPort`].
///
/// Serial terminals need `\r\n` to start a new line, so each `\n` goes out as
/// `\r\n`. [`SerialPort::write_bytes`] itself stays raw, which keeps it usable
/// for binary framing later.
pub struct SerialWriter<'a, P: SerialPort + ?Sized>(pub &'a mut P);

impl<P: SerialPort + ?Sized> fmt::Write for SerialWriter<'_, P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (i, line) in s.split('\n').enumerate() {
            if i > 0 {
                self.0.write_bytes(b"\r\n");
            }
            if !line.is_empty() {
                self.0.write_bytes(line.as_bytes());
            }
        }
        Ok(())
    }
}
