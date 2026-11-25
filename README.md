# Hi5Kernel / Hi5OS v0.0.1 — first boot candidate

Independent x86-64 Rust kernel research for Hi5Central. **Source candidate; not yet compiled or boot-qualified.** No Windows or Linux kernel is linked into the runtime. UEFI firmware loads the initial PE image.

- Original no_std Rust, no external Cargo crates.
- UEFI memory map handoff and bounded ExitBootServices retry.
- Direct COM1 diagnostics and GOP framebuffer drawing after handoff.
- CPUID and conventional-memory accounting; no allocation or reclamation yet.
- Single CPU, interrupts disabled, halt at end.

Boot and kernel code are linked into one EFI PE image. Firmware-initialised page tables, descriptor tables and stack are retained until a later milestone. No scheduler, user mode, general application loader, MSI, network, or production integration yet. Secure Boot is not supported. Test only in an isolated disposable VM.

## Build and test

On a development Ubuntu 24.04 machine with rustup installed:

    sudo apt-get install qemu-system-x86 ovmf mtools dosfstools
    rustup target add x86_64-unknown-uefi
    bash scripts/build.sh
    python3 -m unittest discover -s scripts -p 'test_*.py'
    python3 scripts/smoke.py --memory 256
    python3 scripts/smoke.py --memory 1024

The GitHub Actions workflow builds and tests both VM sizes, retaining the raw FAT32 image, EFI executable, checksums and logs for 14 days. No deployment or production secrets are required. For a local graphical test, run python3 scripts/smoke.py --interactive. The image is not a physical-machine installer or ISO.

## Qualification

Require ordered post-handoff serial markers through KERNEL_READY and no reset or panic. The validator unit tests do not establish successful compilation or VM boot. Verify framebuffer output visually as a separate test.

## Rights and scope

Public repository approved for development builds on 25 November 2026. No open-source licence is granted by this notice; GitHub platform terms apply. Do not publish production secrets, credentials, proprietary third-party code or binaries. Keep this project separate from the existing Hi5Central production environment.

Next: v0.0.2 owned stack, GDT/IDT, physical frame allocator and page tables after boot qualification.
