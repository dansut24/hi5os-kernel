# Hi5OS v0.1.0 — first persistent document candidate

Baseline v0.0.9 5f642be320b8ad34ae753a6c85f060e2401cb97b passed CI at 256/512/1024 MiB; user reports UTM working great. Preserved baseline/v0.0.9. No production changes.

Candidate adds native AHCI DMA sector access on Q35, opt-in data image with dedicated signature, two alternating CRC-protected document slots with flush and readback, a 384-byte ASCII Notepad and cold-restart test. File Explorer opens notes and supports create/delete. Writes restricted to slots 1 and 2 on recognised data disk. Never intentionally write system image. No automatic formatting. Missing/invalid data disk falls back to RAM. Experimental VM only; not general FAT32, directories or arbitrary files. Single active kernel-context window, no network driver/IP, process isolation, USB HID or full keyboard layout. Do not attach physical drives or important data.

UTM: clone stopped known-good VM. Replace only system IDE image with hi5os-v0.1.0.img. Add second imported IDE/SATA disk hi5os-data-v1.img; retain it on subsequent updates. Q35 x86_64, 512 MiB, one core, UEFI, USB Support disabled, Force PS/2 Controller enabled. No sharing/network. Press 5 or F5 for Notepad, type/backspace/Enter, F6 or Ctrl+S saves, Esc closes. A saved document survives reboot only when compatible data disk attached and save succeeds. Never delete existing data disk during upgrades.

Build: bash scripts/build.sh. CI retains system image, initial empty data image, checksums, font notice, logs/screenshots; tests cold restart with a temporary data image. Candidate pending CI/manual UTM. No open-source licence granted by this notice; bundled glyph font terms apply separately.
