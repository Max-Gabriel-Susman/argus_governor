//! Serial output through the firmware's Serial I/O protocol.

use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol};
use uefi::proto::console::serial::Serial;

use super::SerialPort;

/// The first serial port the firmware knows about: COM1 on QEMU's q35.
///
/// Boot services only. Drop it before exiting them, because dropping it
/// closes the protocol, and closing is itself a boot service.
pub struct FirmwareSerial {
    port: ScopedProtocol<Serial>,
}

impl FirmwareSerial {
    pub fn open() -> uefi::Result<Self> {
        let handle = boot::get_handle_for_protocol::<Serial>()?;

        // Opened shared (GET_PROTOCOL), not exclusive. An exclusive open would
        // disconnect OVMF's console driver from the port, and ConOut would
        // stop reaching the serial line.
        //
        // SAFETY: boot services run on one CPU without threads, so the console
        // driver's writes and ours never interleave, and nothing uninstalls
        // the protocol while we hold it.
        let port = unsafe {
            boot::open_protocol::<Serial>(
                OpenProtocolParams {
                    handle,
                    agent: boot::image_handle(),
                    controller: None,
                },
                OpenProtocolAttributes::GetProtocol,
            )?
        };

        Ok(Self { port })
    }
}

impl SerialPort for FirmwareSerial {
    fn write_bytes(&mut self, bytes: &[u8]) {
        // write_exact retries short writes and only gives up on a dead port.
        // A greeting that can't be sent isn't worth stopping the boot over.
        let _ = self.port.write_exact(bytes);
    }
}
