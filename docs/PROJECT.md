# Hi5OS handoff — 25 November 2026

Goal: independent Rust-first Hi5Kernel, proprietary Hi5Office and independently implemented EXE/MSI compatibility. Public hi5os-kernel approved for CI; no production RMM/ITSM/StreamCore or EU environment changes.

Baseline v0.0.1 cc3ae6aac6e891d1e7484c1fb53d7701dd6618db passed CI 256/1024 MiB and user visual UTM SE iOS 512 MiB. Preserved on baseline/v0.0.1 and main. Candidate v0.0.2 on dev/v0.0.2: independent 64 KiB stack, allocator bitmap, direct frame tests and diagnostic display. CI tests 256/512/1024 MiB. Next: qualify on UTM SE, then owned page tables, GDT/IDT and fault/guard-page tests; do not claim memory isolation before testing it. Later scheduler, user mode, storage, network, desktop, PE loader and Win32/MSI. No production changes.
