#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for tool in cargo mkfs.vfat mmd mcopy; do command -v "$tool" >/dev/null || { echo "Missing $tool" >&2; exit 1; }; done
/usr/bin/python3 scripts/fontgen.py
cargo build --release --locked --target x86_64-unknown-uefi
mkdir -p dist
cp target/x86_64-unknown-uefi/release/hi5kernel.efi dist/BOOTX64.EFI
# A removable UEFI FAT32 image, not a physical-machine installer or an ISO.
rm -f dist/hi5os-v0.1.7.img
truncate -s 64M dist/hi5os-v0.1.7.img
mkfs.vfat -F 32 -n HI5OS dist/hi5os-v0.1.7.img
mmd -i dist/hi5os-v0.1.7.img ::/EFI ::/EFI/BOOT
mcopy -i dist/hi5os-v0.1.7.img dist/BOOTX64.EFI ::/EFI/BOOT/BOOTX64.EFI
rm -f dist/hi5os-data-v1.img
python3 scripts/data_image.py dist/hi5os-data-v1.img
(cd dist && sha256sum BOOTX64.EFI hi5os-v0.1.7.img hi5os-data-v1.img > SHA256SUMS)
