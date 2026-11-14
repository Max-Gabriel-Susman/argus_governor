#!/usr/bin/env bash
# Cargo runner for x86_64-unknown-uefi. `cargo run` calls it as
#
#     scripts/qemu-run.sh <path/to/governor_loader.efi> [extra QEMU args...]
#
# and it boots that .efi the way a PC boots a USB stick:
#   1. copies it to <profile dir>/esp/EFI/BOOT/BOOTX64.EFI, the file UEFI
#      loads from removable media when no boot entry says otherwise;
#   2. makes a fresh, writable copy of the OVMF variable store;
#   3. starts QEMU on q35 with OVMF (no Secure Boot), serial on this terminal,
#      KVM when /dev/kvm is usable and TCG otherwise.
#
# Firmware paths can be overridden: OVMF_CODE=... OVMF_VARS=... cargo run

set -euo pipefail

efi="${1:?usage: qemu-run.sh <path/to/app.efi> [extra QEMU args...]}"
shift

out_dir="$(cd "$(dirname "$efi")" && pwd)"
esp="$out_dir/esp"
vars="$out_dir/OVMF_VARS.fd"

# ---- Firmware ---------------------------------------------------------------
# Ubuntu's ovmf package: OVMF_CODE_4M.fd is the build without Secure Boot and
# OVMF_VARS_4M.fd is its empty variable store. 22.04 also ships 2 MB versions
# without the _4M suffix. Code and vars must come from the same pair.
if [[ -z "${OVMF_CODE:-}" || -z "${OVMF_VARS:-}" ]]; then
    for suffix in _4M ""; do
        if [[ -f "/usr/share/OVMF/OVMF_CODE$suffix.fd" && -f "/usr/share/OVMF/OVMF_VARS$suffix.fd" ]]; then
            OVMF_CODE="/usr/share/OVMF/OVMF_CODE$suffix.fd"
            OVMF_VARS="/usr/share/OVMF/OVMF_VARS$suffix.fd"
            break
        fi
    done
fi
if [[ ! -f "${OVMF_CODE:-}" || ! -f "${OVMF_VARS:-}" ]]; then
    echo "qemu-run: OVMF firmware not found." >&2
    echo "  Install it:  sudo apt install ovmf" >&2
    echo "  Then check:  dpkg -L ovmf | grep -E 'OVMF_(CODE|VARS)'" >&2
    echo "  and rerun with OVMF_CODE=<code .fd> OVMF_VARS=<vars .fd> if the names differ." >&2
    exit 2
fi

# ---- ESP and variable store -------------------------------------------------
mkdir -p "$esp/EFI/BOOT"
cp "$efi" "$esp/EFI/BOOT/BOOTX64.EFI"
# A fresh copy every run: no boot entries carried over, so every run starts
# from the same firmware state. That is what scripted runs and CI want.
install -m 644 "$OVMF_VARS" "$vars"

# ---- Accelerator and CPU ----------------------------------------------------
# Skylake-Client-v4: AVX2 but no AVX-512, TSX or MPX. Every feature it exposes
# is one the Atom x7425E (Gracemont) has too. See the README.
cpu=Skylake-Client-v4
if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    accel=kvm
else
    accel=tcg
    # TCG can't emulate a few of the model's features (pcid, x2apic, xsavec and
    # others, depending on the QEMU version) and drops them either way.
    # check=off only stops it printing a warning per feature per vCPU.
    cpu+=",check=off"
    echo "qemu-run: /dev/kvm is not usable, so QEMU falls back to TCG (works, just slower)." >&2
    if [[ -e /dev/kvm ]]; then
        echo "  Fix: sudo usermod -aG kvm \"\$USER\", then log out and back in." >&2
    else
        echo "  Fix: enable VT-x (Intel) or SVM (AMD) in the PC's firmware setup." >&2
        echo "  'sudo apt install cpu-checker && kvm-ok' reports what is missing." >&2
    fi
fi

# ---- QEMU -------------------------------------------------------------------
# -nodefaults: no default VGA, NIC, CD-ROM or monitor. The VM has only what is
# listed here, so the firmware never tries a PXE or DVD boot first, and the
# boot log is the same on QEMU 6.2 (Ubuntu 22.04) and 8.2 (Ubuntu 24.04).
qemu_args=(
    -nodefaults
    -machine q35
    -accel "$accel"
    -cpu "$cpu"
    -smp 4
    -m 512M
    -drive "if=pflash,format=raw,unit=0,readonly=on,file=$OVMF_CODE"
    -drive "if=pflash,format=raw,unit=1,file=$vars"
    -drive "format=raw,file=fat:rw:$esp"
    -display none
    -serial stdio
    -no-reboot
    -device "isa-debug-exit,iobase=0xf4,iosize=0x04"
)

exec qemu-system-x86_64 "${qemu_args[@]}" "$@"
