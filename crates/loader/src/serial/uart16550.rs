//! A 16550-compatible UART, driven directly.
//!
//! The driver only needs to read and write the UART's eight registers. How it
//! reaches them is a separate [`Registers`] implementation: [`PortIo`] for a
//! PC's COM1 at I/O port 0x3F8, and later a memory-mapped one if the board's
//! UART header is wired to a SoC UART instead.

use super::SerialPort;
use crate::port;

/// I/O base of COM1 on a PC.
pub const COM1_BASE: u16 = 0x3F8;

/// Divisor for 115200 baud with a PC UART's 1.8432 MHz clock.
pub const BAUD_115200: u16 = 1;

// Register offsets. With LCR.DLAB set, offsets 0 and 1 hold the divisor.
const THR: u8 = 0; // transmit holding register (write); divisor low byte with DLAB
const IER: u8 = 1; // interrupt enable; divisor high byte with DLAB
const FCR: u8 = 2; // FIFO control (write)
const LCR: u8 = 3; // line control
const MCR: u8 = 4; // modem control
const LSR: u8 = 5; // line status

const LCR_DLAB: u8 = 0x80; // divisor latch access
const LCR_8N1: u8 = 0x03; // 8 data bits, no parity, 1 stop bit
const FCR_ENABLE_AND_CLEAR: u8 = 0xC7; // FIFOs on, both cleared, 14-byte RX trigger
const MCR_DTR_RTS: u8 = 0x03;
const LSR_THR_EMPTY: u8 = 0x20; // the UART can take another byte

/// How the driver reaches the UART's registers, numbered 0 to 7.
pub trait Registers {
    fn read(&mut self, register: u8) -> u8;
    fn write(&mut self, register: u8, value: u8);
}

/// Registers at eight consecutive I/O ports, as on a PC's COM ports.
pub struct PortIo {
    base: u16,
}

impl PortIo {
    /// # Safety
    ///
    /// `base` must be the first of the eight I/O ports of a 16550-compatible
    /// UART, and nothing else may drive that UART while this value exists.
    pub const unsafe fn new(base: u16) -> Self {
        Self { base }
    }
}

impl Registers for PortIo {
    fn read(&mut self, register: u8) -> u8 {
        // SAFETY: new()'s contract says these ports are a UART we own.
        unsafe { port::inb(self.base + u16::from(register)) }
    }

    fn write(&mut self, register: u8, value: u8) {
        // SAFETY: as above.
        unsafe { port::outb(self.base + u16::from(register), value) }
    }
}

/// A 16550 set up for polled transmit: no interrupts, FIFOs on, 8N1.
pub struct Uart16550<R: Registers> {
    regs: R,
}

impl<R: Registers> Uart16550<R> {
    /// Programs the UART. `divisor` is the UART clock divided by 16 × baud,
    /// so [`BAUD_115200`] for a PC's COM1.
    pub fn new(mut regs: R, divisor: u16) -> Self {
        let [divisor_low, divisor_high] = divisor.to_le_bytes();
        regs.write(IER, 0x00); // no interrupts: transmit is polled
        regs.write(LCR, LCR_DLAB); // offsets 0 and 1 become the divisor
        regs.write(THR, divisor_low);
        regs.write(IER, divisor_high);
        regs.write(LCR, LCR_8N1); // clears DLAB again
        regs.write(FCR, FCR_ENABLE_AND_CLEAR);
        regs.write(MCR, MCR_DTR_RTS);
        Self { regs }
    }
}

impl<R: Registers> SerialPort for Uart16550<R> {
    fn write_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            // Wait until the transmitter can take a byte. A missing UART reads
            // as 0xFF, which has this bit set, so this can't hang on one.
            while self.regs.read(LSR) & LSR_THR_EMPTY == 0 {
                core::hint::spin_loop();
            }
            self.regs.write(THR, byte);
        }
    }
}
