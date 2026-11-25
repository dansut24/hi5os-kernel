#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for tool in cargo mkfs.vfat mmd mcopy clang lld-link; do command -v "$tool" >/dev/null || { echo "Missing $tool" >&2; exit 1; }; done
/usr/bin/python3 scripts/fontgen.py
mkdir -p target/hi5-fixture
/usr/bin/python3 scripts/prepare7zip.py
clang --target=x86_64-pc-windows-msvc -c fixtures/hello.s -o target/hi5-fixture/hello.obj
lld-link /lib /def:fixtures/kernel32.def /machine:x64 /out:target/hi5-fixture/kernel32.lib
lld-link /entry:hi5_entry /subsystem:console /nodefaultlib /fixed /base:0x100000000 /filealign:512 /timestamp:0 /out:target/hi5-fixture/hello.exe target/hi5-fixture/hello.obj target/hi5-fixture/kernel32.lib
clang --target=x86_64-pc-windows-msvc -c fixtures/runtime.s -o target/hi5-fixture/runtime.obj
lld-link /entry:hi5_entry /subsystem:console /nodefaultlib /fixed /base:0x100000000 /filealign:512 /timestamp:0 /out:target/hi5-fixture/runtime.exe target/hi5-fixture/runtime.obj target/hi5-fixture/kernel32.lib
clang --target=x86_64-pc-windows-msvc -c fixtures/console.s -o target/hi5-fixture/console.obj
lld-link /entry:hi5_entry /subsystem:console /nodefaultlib /fixed /base:0x100000000 /filealign:512 /timestamp:0 /out:target/hi5-fixture/console.exe target/hi5-fixture/console.obj target/hi5-fixture/kernel32.lib
cargo build --release --locked --target x86_64-unknown-uefi
mkdir -p dist
cp target/hi5-fixture/hello.exe dist/hi5-winhello.exe
cp target/hi5-fixture/runtime.exe dist/hi5-winruntime.exe
cp target/hi5-fixture/console.exe dist/hi5-winconsole.exe
cp target/x86_64-unknown-uefi/release/hi5kernel.efi dist/BOOTX64.EFI
rm -rf dist/7zip-notices
cp -r target/hi5-fixture/7zip-notices dist/7zip-notices
# A removable UEFI FAT32 image, not a physical-machine installer or an ISO.
rm -f dist/hi5os-v0.2.6.img
truncate -s 64M dist/hi5os-v0.2.6.img
mkfs.vfat -F 32 -n HI5OS dist/hi5os-v0.2.6.img
mmd -i dist/hi5os-v0.2.6.img ::/EFI ::/EFI/BOOT
mcopy -i dist/hi5os-v0.2.6.img dist/BOOTX64.EFI ::/EFI/BOOT/BOOTX64.EFI
mcopy -s -i dist/hi5os-v0.2.6.img dist/7zip-notices ::/7ZIP-NOTICES
rm -f dist/hi5os-data-v1.img
python3 scripts/data_image.py dist/hi5os-data-v1.img
(cd dist && sha256sum hi5-winhello.exe hi5-winruntime.exe hi5-winconsole.exe BOOTX64.EFI hi5os-v0.2.6.img hi5os-data-v1.img > SHA256SUMS)
