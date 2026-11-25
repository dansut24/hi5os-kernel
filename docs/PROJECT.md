# Hi5OS project handoff

25 November 2026. First boot source candidate, not released or qualified.

Goal: proprietary Hi5Central desktop OS with independent Rust-first hybrid kernel, native applications, independently implemented Windows EXE/MSI compatibility and Hi5Office. Compatibility is a staged objective, not a guarantee.

Environment: separate hi5os-kernel public repository (explicitly authorised for CI), x86-64 QEMU + OVMF. No EU environment, no production VPS/RMM/StreamCore or physical-device changes. Keep privileged kernel minimal; RMM, identity, networking services, UI and compatibility layers generally in user mode. Review dependency licences and source provenance; no copied proprietary Windows source or binaries.

Candidate: UEFI ABI bindings, memory-map handoff, direct COM1, direct GOP logo, CPUID and idle halt. Rust 1.85.1 x86_64-unknown-uefi, no external Cargo crates. FAT32 image script and CI smoke harness. Compilation and boot evidence pending.

Next: qualify v0.0.1 before owned memory/GDT/IDT, scheduler, isolated user mode, storage, networking, compositor, PE loader, Win32 APIs, MSI engine, Hi5Central integration and Hi5Office.

References: https://uefi.org/specs/UEFI/2.10/04_EFI_System_Table.html ; https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html ; https://doc.rust-lang.org/stable/rustc/platform-support/unknown-uefi.html .
