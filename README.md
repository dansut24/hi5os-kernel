# Hi5OS v0.0.9 — native desktop refinement candidate

Known-good v0.0.8 commit 9682e21a1e8667aabb9fae25d5b9e41a289f104b passed QEMU 256/512/1024 MiB and user UTM SE iOS desktop and keyboard. Preserve baseline/v0.0.8.

UTM SE: x86_64 Q35, 512 MiB, one core, UEFI, IDE image; Input > USB Support Disabled, QEMU > Force PS/2 Controller enabled. User confirmed keyboard works. No network/sharing. Use a stopped duplicate VM.

v0.0.9: original native Hi5UI rasteriser, build-time antialiased DejaVu Sans ASCII glyph masks (font copyright notice in artifact), bounded 2048x1536-pixel backbuffer capacity, light/dark appearance, desktop icons, Start menu, taskbar, single foreground window with close/minimise/maximise/restore and experimental mouse drag on release, Settings, RAM File Explorer, Network discovery, and an interactive command terminal. Keyboard 1-4 (or F1-F4), M Start, T theme, X maximise/restore, Esc close, N/D file actions. Terminal help, mem, ver, ls, clear, theme; Esc closes, F1-F4 switch. Basic ASCII key translation, not full UK layout or Unicode.

Build: install rustup and apt packages qemu-system-x86 ovmf mtools dosfstools python3-pil fonts-dejavu-core; bash scripts/build.sh. CI uses QMP keyboard commands and captures PPM screenshots. No font runtime parser or GPU acceleration. No persistent files, NIC driver/IP, general process isolation, USB HID, simultaneous windows or freely resizable windows. All desktop apps run in kernel context as before. No production changes or physical hardware use. Candidate unqualified until CI and UTM pass. No open-source licence granted by this notice; third-party font terms apply separately.
