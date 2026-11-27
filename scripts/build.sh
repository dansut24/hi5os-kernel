#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for tool in cargo mkfs.vfat mmd mcopy clang lld-link curl; do command -v "$tool" >/dev/null || { echo "Missing $tool" >&2; exit 1; }; done
/usr/bin/python3 scripts/fontgen.py
/usr/bin/python3 scripts/shell_art.py
mkdir -p target/hi5-fixture
/usr/bin/python3 scripts/prepare7zip.py
/usr/bin/python3 scripts/prepare_gui.py
clang --target=x86_64-pc-windows-msvc -c fixtures/hello.s -o target/hi5-fixture/hello.obj
lld-link /lib /def:fixtures/kernel32.def /machine:x64 /out:target/hi5-fixture/kernel32.lib
lld-link /entry:hi5_entry /subsystem:console /nodefaultlib /fixed /base:0x100000000 /filealign:512 /timestamp:0 /out:target/hi5-fixture/hello.exe target/hi5-fixture/hello.obj target/hi5-fixture/kernel32.lib
clang --target=x86_64-pc-windows-msvc -c fixtures/runtime.s -o target/hi5-fixture/runtime.obj
lld-link /entry:hi5_entry /subsystem:console /nodefaultlib /fixed /base:0x100000000 /filealign:512 /timestamp:0 /out:target/hi5-fixture/runtime.exe target/hi5-fixture/runtime.obj target/hi5-fixture/kernel32.lib
clang --target=x86_64-pc-windows-msvc -c fixtures/console.s -o target/hi5-fixture/console.obj
lld-link /entry:hi5_entry /subsystem:console /nodefaultlib /fixed /base:0x100000000 /filealign:512 /timestamp:0 /out:target/hi5-fixture/console.exe target/hi5-fixture/console.obj target/hi5-fixture/kernel32.lib
clang --target=x86_64-pc-windows-msvc -ffreestanding -fno-stack-protector -O1 -c fixtures/h5dll.c -o target/hi5-fixture/h5dll.obj
lld-link /dll /noentry /nodefaultlib /dynamicbase /base:0x180000000 /timestamp:0 /out:target/hi5-fixture/h5dll.dll /implib:target/hi5-fixture/h5dll.lib target/hi5-fixture/h5dll.obj
clang --target=x86_64-pc-windows-msvc -ffreestanding -fno-stack-protector -O1 -c fixtures/h5gui.c -o target/hi5-fixture/h5gui.obj
lld-link /lib /def:fixtures/user32-gui.def /machine:x64 /out:target/hi5-fixture/user32-gui.lib
lld-link /lib /def:fixtures/gdi32-gui.def /machine:x64 /out:target/hi5-fixture/gdi32-gui.lib
lld-link /entry:hi5_entry /subsystem:windows /nodefaultlib /dynamicbase /base:0x140000000 /timestamp:0 /out:target/hi5-fixture/h5gui.exe target/hi5-fixture/h5gui.obj target/hi5-fixture/h5dll.lib target/hi5-fixture/user32-gui.lib target/hi5-fixture/gdi32-gui.lib
/usr/bin/python3 scripts/prepare_tls.py
/usr/bin/python3 scripts/prepare_image.py
/usr/bin/python3 scripts/prepare_wasmi.py
/usr/bin/python3 scripts/prepare_js.py
RUSTFLAGS="-Lnative=$PWD/target/hi5-tls -Lnative=$PWD/target/hi5-image" cargo build --release --locked --target x86_64-unknown-uefi
mkdir -p dist
/usr/bin/python3 scripts/prepare_js.py --notices
rm -rf dist/hi5-js-notices
cp -r target/hi5-js/notices dist/hi5-js-notices
cp target/hi5-fixture/gui-package.json dist/7zip-gui-package.json
cp target/hi5-fixture/h5gui.exe dist/hi5-wingui.exe
cp target/hi5-fixture/h5dll.dll dist/hi5-test.dll
cp target/hi5-fixture/hello.exe dist/hi5-winhello.exe
cp target/hi5-fixture/runtime.exe dist/hi5-winruntime.exe
cp target/hi5-fixture/console.exe dist/hi5-winconsole.exe
cp target/x86_64-unknown-uefi/release/hi5kernel.efi dist/BOOTX64.EFI
rm -rf dist/7zip-notices
cp -r target/hi5-fixture/7zip-notices dist/7zip-notices
cp target/hi5-tls/bearssl-0.6/LICENSE.txt dist/BEARSSL-LICENSE.txt
cp target/hi5-image/STB-LICENSE.txt dist/STB-LICENSE.txt
# A removable UEFI FAT32 image, not a physical-machine installer or an ISO.
rm -f dist/hi5os-v0.3.4.img
truncate -s 64M dist/hi5os-v0.3.4.img
mkfs.vfat -F 32 -n HI5OS dist/hi5os-v0.3.4.img
mmd -i dist/hi5os-v0.3.4.img ::/EFI ::/EFI/BOOT
mcopy -i dist/hi5os-v0.3.4.img dist/BOOTX64.EFI ::/EFI/BOOT/BOOTX64.EFI
mcopy -s -i dist/hi5os-v0.3.4.img dist/7zip-notices ::/7ZIP-NOTICES
mcopy -s -i dist/hi5os-v0.3.4.img dist/hi5-js-notices ::/HI5-JS-NOTICES
rm -f dist/hi5os-data-v1.img
python3 scripts/data_image.py dist/hi5os-data-v1.img
rm -f dist/hi5os-setup-data.img
python3 scripts/data_image.py dist/hi5os-setup-data.img --setup
(cd dist && sha256sum hi5-wingui.exe hi5-test.dll hi5-winhello.exe hi5-winruntime.exe hi5-winconsole.exe BOOTX64.EFI hi5os-v0.3.4.img hi5os-data-v1.img hi5os-setup-data.img > SHA256SUMS)
