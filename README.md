# argus_governor

The edge node of Argus Cybernetics: a bare-metal Rust controller for the Intel Atom x7425E (AAEON UP Squared Pro 7000) that will turn decoded intent from the Argus pipeline into bounded commands for an industrial site.

There is no operating system on the node. The boot chain is:

```text
UEFI firmware → governor_loader (UEFI app, uefi crate) → Embassy application (x86_64-unknown-none)
```

Development happens in QEMU first, on a q35 machine with OVMF firmware.

## Status

| Milestone | What runs | State |
| --- | --- | --- |
| 1. UEFI hello | Greeting on the UEFI console and the serial port, then power off | Done |
| 2. Off the firmware | Exit boot services, print through the 16550 UART directly, end QEMU through isa-debug-exit | Next |
| 3. Embassy hello | One Embassy task on a raw executor, spin-polled | Planned |

## Layout

```text
argus_governor/
├── .cargo/config.toml          build target and the QEMU runner
├── Cargo.toml                  workspace and pinned dependency versions
├── Cargo.lock
├── rust-toolchain.toml         pinned stable toolchain plus the UEFI target
├── crates/
│   └── loader/                 governor_loader, booted as BOOTX64.EFI
│       └── src/
│           ├── main.rs
│           ├── serial.rs           SerialPort trait and the write!/writeln! adapter
│           └── serial/
│               └── firmware.rs     SerialPort over the UEFI Serial I/O protocol
└── scripts/
    └── qemu-run.sh             cargo runner: lays out the ESP and starts QEMU
```

`crates/kernel` (the Embassy application) and `crates/proto` (types shared with the host side) come later.

## Setup (Ubuntu)

```bash
sudo apt install qemu-system-x86 ovmf
# rustup itself, if it isn't installed yet: https://rustup.rs
rustup toolchain install 1.100.0 --profile minimal \
    --target x86_64-unknown-uefi --component rustfmt,clippy
```

## Build and run

```bash
cargo run              # debug build
cargo run --release    # optimized build, about 19 KiB
```

`cargo run` builds `governor_loader.efi` and passes its path to `scripts/qemu-run.sh`, the runner set in `.cargo/config.toml`. The script copies the file to `target/x86_64-unknown-uefi/<profile>/esp/EFI/BOOT/BOOTX64.EFI`, puts a fresh writable copy of the OVMF variable store next to it, and boots QEMU with that `esp` directory as a FAT drive and the serial port on your terminal. Arguments after `--` go to QEMU, for example `cargo run -- -d cpu_reset -D qemu.log`.

Expected output, without the firmware's terminal escape sequences:

```text
BdsDxe: loading Boot0001 "UEFI QEMU HARDDISK QM00001 " from PciRoot(0x0)/Pci(0x1F,0x2)/Sata(0x0,0xFFFF,0x0)
BdsDxe: starting Boot0001 "UEFI QEMU HARDDISK QM00001 " from PciRoot(0x0)/Pci(0x1F,0x2)/Sata(0x0,0xFFFF,0x0)
Hello, Atom! (UEFI)
Hello, Atom! (UEFI)
```

The first greeting is the UEFI console (ConOut), which OVMF routes to COM1 through its terminal driver. The second goes through the Serial I/O protocol. QEMU then exits with status 0, because the app asks the firmware to power off.

## How it boots

1. QEMU starts OVMF from the read-only code flash. OVMF keeps its settings in the separate, writable variable flash.
2. With a fresh variable store, OVMF's boot manager creates an entry for each device. For a disk with no boot entry of its own, it loads the fallback path `\EFI\BOOT\BOOTX64.EFI`. QEMU presents the `esp` directory as that disk.
3. OVMF loads the PE image and calls its entry point with the image handle and the system table. The uefi crate's `#[entry]` stores both, and `main` runs with boot services (console, protocols, memory allocation) available.
4. Milestone 1 ends there: `main` prints, then calls the runtime service ResetSystem to power off.
5. From milestone 2 on, the loader calls ExitBootServices. The firmware stops its drivers and timer and hands the machine over. From then on, only code in this repo drives the hardware, starting with the UART.

## QEMU settings

- `-machine q35`: a PCIe chipset with memory-mapped PCI configuration space, which virtio-drivers will need.
- OVMF from Ubuntu's `ovmf` package: `OVMF_CODE_4M.fd` (the build without Secure Boot), read-only, plus a writable copy of `OVMF_VARS_4M.fd`. If the 4M pair is missing, the script uses the 2 MB `OVMF_CODE.fd` and `OVMF_VARS.fd`. Check your names with `dpkg -L ovmf | grep -E 'OVMF_(CODE|VARS)'` and override with `OVMF_CODE=... OVMF_VARS=... cargo run`.
- `-accel kvm` when `/dev/kvm` is readable and writable, TCG otherwise.
- `-smp 4 -m 512M`: four cores, like the x7425E. Only the first one runs our code so far.
- `-nodefaults`: no default VGA, NIC, CD-ROM or monitor, so the VM has only the devices the script lists. The firmware never tries a PXE or DVD boot first, and the boot log is the same on QEMU 6.2 and 8.2. virtio-net gets added explicitly later.
- `-display none -serial stdio -no-reboot`: COM1 on the terminal, and any reset, including a triple fault, ends QEMU instead of looping.
- `-device isa-debug-exit,iobase=0xf4,iosize=0x04`: lets the guest end QEMU with a status it picks. Writing `v` to port `0xf4` makes QEMU exit with `(v << 1) | 1`. Unused until milestone 2.

### CPU model: `Skylake-Client-v4`

The x7425E's Gracemont cores support AVX2, FMA, BMI1/2, ADX, RDSEED, SHA, GFNI, VAES and AVX-VNNI, but not AVX-512, TSX or MPX. The guest should only see features from that set, so anything that runs in QEMU also runs on the board. `-cpu host` and `-cpu max` fail that test on any host with AVX-512.

`Skylake-Client-v4` is the newest Intel model in `qemu-system-x86_64 -cpu help` that passes:

- Every feature it exposes (AVX2, FMA, BMI1/2, ADX, RDSEED, XSAVEC/XSAVES, SMEP/SMAP, INVPCID and the baseline ones) is in the CPU flags of an Alder Lake-N part.
- Version 3 and later turn TSX off, and the model has no MPX.
- QEMU 6.2 (Ubuntu 22.04) and 8.2 (Ubuntu 24.04) define it identically. The versioned name pins it; the bare `Skylake-Client` alias can change with the machine type.

Rejected: Icelake-Server, Cascadelake-Server, Cooperlake, SapphireRapids and GraniteRapids (AVX-512); SierraForest (newer E-core instructions such as AVX-IFMA and CMPCCXADD); Snowridge (CLDEMOTE, which Gracemont lacks, and no AVX); Denverton (a safe subset, but it stops at SSE4.2). The model under-approximates the real chip, which is the safe direction.

The Rust target's default CPU features are left alone. `x86_64-unknown-uefi` compiles with soft-float and no SSE, so the code itself uses no SIMD at all.

Under TCG, QEMU can't emulate some of the model's features (PCID, x2APIC, XSAVEC and others, depending on the version) and drops them. The script adds `check=off` in that case, so QEMU doesn't print one warning per feature per core.

## Troubleshooting

**`qemu-run: OVMF firmware not found.`**
Run `sudo apt install ovmf`. If it's installed under other names, find them with `dpkg -L ovmf | grep -E 'OVMF_(CODE|VARS)'` and set `OVMF_CODE` and `OVMF_VARS`.

**`Could not open '/usr/share/OVMF/OVMF_VARS_4M.fd': Permission denied`**
QEMU was given the packaged variable store, which is read-only for users. It needs a writable copy, which the script makes for you.

**`error[E0463]: can't find crate for 'core'` with `the 'x86_64-unknown-uefi' target may not be installed`**
Run `rustup target add x86_64-unknown-uefi --toolchain 1.100.0`, or rerun the toolchain install above.

**No output for a few seconds**
That's normal under TCG, where the firmware takes a few seconds to reach the boot manager.

**The terminal clears, or the output has stray characters**
OVMF's terminal driver sends clear-screen and cursor escapes over serial. Scroll back, or capture the run with `cargo run > boot.log` and read it with `cat -v boot.log`.

**No output at all**
Start the run with `cargo run`, because the runner supplies `-serial stdio`. If you start QEMU by hand, keep `-serial stdio` and don't add `-nographic`, since both claim the terminal. `cargo run -- -d cpu_reset,guest_errors -D qemu.log` records resets and faults.

**`BdsDxe: failed to load Boot0001 ... Not Found`, then a `Shell>` prompt**
The loader isn't at `EFI/BOOT/BOOTX64.EFI` on the drive. At the shell, type `fs0:` and then `ls EFI\BOOT` to see what is there. Ctrl-C quits QEMU.

**`BdsDxe: failed to load Boot0001 ... Access Denied`, then `No bootable option or device was found`**
Secure Boot is on, so the firmware refuses the unsigned loader. A `secboot` code file or the `.ms` or `snakeoil` variable store was used. Use `OVMF_CODE_4M.fd` with `OVMF_VARS_4M.fd`. On the real board, turn Secure Boot off in the firmware setup.

**`qemu-run: /dev/kvm is not usable`**
If `/dev/kvm` exists, run `sudo usermod -aG kvm "$USER"`, log out and back in, and check with `groups`. If it doesn't exist, enable VT-x in the PC's firmware setup. `kvm-ok` from the `cpu-checker` package tells you which. TCG works in the meantime.

**`warning: host doesn't support requested feature: ...` under KVM**
The host CPU lacks one of the model's features, and QEMU drops it. That's harmless here.

## Later

The same `BOOTX64.EFI` will boot the UP Squared Pro 7000 from a FAT32 USB stick, at `EFI/BOOT/BOOTX64.EFI`, with its console on the board's UART header at 115200 baud.

## License

Apache-2.0
