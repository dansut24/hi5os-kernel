# Hi5Kernel v0.0.2 — memory milestone candidate

Known-good v0.0.1: commit cc3ae6aac6e891d1e7484c1fb53d7701dd6618db, baseline/v0.0.1. QEMU 256/1024 MiB passed, visual UTM SE iOS 512 MiB x86_64 Q35 passed on 25 November 2026.

This branch adds a 64 KiB kernel-owned stack and canary; conservative 4 KiB frame allocation using separate managed/free bitmaps, only EFI conventional memory from 1 MiB to 4 GiB (excluding framebuffer); writable identity mapping checks; eight-frame pattern read/write, distinctness, reuse, invalid/double-free and free-count tests; on-screen version, progress, memory and CPUID diagnostics.

Still uses firmware-initialised page tables and descriptor tables; no guard pages, allocator concurrency, exception isolation, scheduling, user mode, EXE/MSI or filesystem. This is the first staged part of memory development. No production or physical machine use, no secrets, no open-source licence granted by this notice. No external Rust dependencies.

Build with bash scripts/build.sh after installing rustup, qemu-system-x86, ovmf, mtools and dosfstools. Run python3 -m unittest discover -s scripts -p 'test_*.py' and python3 scripts/smoke.py --memory 512. CI also tests 256 and 1024 MiB. The raw FAT32 image is for UEFI VM boot, not installation.

UTM SE: create a separate VM or replace only the imported Hi5OS drive on a stopped copy of the known-good VM; x86_64 Q35, 512 MiB, one core, UEFI enabled, no sharing or networking. Expected screen KERNEL READY with green test lines. Preserve the old VM. Candidate qualification pending.
