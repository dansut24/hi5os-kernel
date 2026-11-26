# Hi5Browser — native full browser development
Decision 2026-11-26: Hi5OS ships a first-party full browser, not merely an HTTP preview. Users retain default-browser choice and may install Chrome/Firefox or other Windows browsers when separately qualified. Feature parity is a long-term acceptance target, not a promise of current compatibility. Native browser work is isolated in dev/hi5browser, starting at qualified CI build 65; manually qualified build 61 remains untouched. Coordinate platform work with dev/v0.3.4; no production deployment.

## Present baseline
Rust freestanding UEFI kernel, framebuffer desktop, restricted HTTP transport and 4 MiB preview / 1 MiB save, no validated TLS or general browser engine. The pinned-PE startup probe is not a general-purpose sandbox or process scheduler. Do not execute arbitrary web content in kernel or in the PE probe.

## Engine decision
Prefer evaluating Servo as a native embeddable Rust engine; assess Chromium/Blink and Gecko for compatibility/performance, dependencies, maintenance and porting cost. Do not commit to a fork until a reproducible port feasibility spike succeeds. Servo uses hosted Rust, native dependencies, JavaScript and graphics; a no_std kernel cannot link it as-is. Chromium or Gecko may be better for mature website compatibility if a sustainable port is feasible. Keep an engine-neutral native UI and versioned IPC. No remote-rendering or Windows VM substituted for native operation. Keep existing HTTP preview labelled diagnostic until secure navigation is ready.
References: https://book.servo.org/embedding/overview.html ; https://servo.org/ ; https://new.chromium.org/developers/design-documents/multi-process-architecture/ ; https://github.com/chromium/chromium/blob/main/docs/design/sandbox.md

## Shared platform gates
1. Protected native user processes with separate address spaces, preemptible threads, virtual memory (map/protect/unmap), process lifecycle, IPC, handles and bounded resource accounting; enforce W^X, crash containment and no privileged renderer.
2. General filesystem with scoped storage, permissions, file locking and atomic writes; socket streams, DNS, IPv4/IPv6, timers, trusted time and hardware-backed entropy. Implement verified TLS with hostname/chain/time validation, secure root updates and no downgrade or insecure fallback. Reuse docs/TLS.md and audited library rather than hand-writing crypto.
3. Native executable loader and hosted Rust/C/C++ runtime, thread-local storage, compiler toolchain/target, dependency build and reproducible CI. Graphics surfaces, compositor, input, scalable Unicode fonts/text shaping and audio/media interfaces.
4. Engine port spike: pinned upstream engine and dependency manifests, license/SBOM, platform adapter, offline HTML/CSS/JS fixture in a separate native process. Prove restart after crash and unauthorized filesystem/network access denied. Evaluate Web Platform Tests results before engine selection.

## Browser application architecture
Privileged broker owns window chrome, navigation policy, profiles and permission prompts; separate least-privilege renderer processes and per-site isolation; network and storage broker with scoped IPC; compositor/GPU and media workers with restricted resources. Renderers cannot directly read user files or credentials or open arbitrary sockets; no unrestricted broker messages. Secure defaults: same-origin policy, CSP, CORS, cookie isolation, mixed-content blocking, private-session storage isolation, explicit camera/mic/location permissions. Signed updates, upstream CVE tracking, crash recovery. Security review before enabling arbitrary internet pages.

## Functional release gates
A: Native UI shell (tabs, address/search, back/forward/reload, history, bookmarks, downloads) on local fixtures; no pretending HTTPS works.
B: Secure networking, HTML/CSS/JS/DOM, forms, images, fonts, scrolling, storage/cookies, navigation and basic DevTools. Cert failure, invalid entropy/time and malicious input tests mandatory.
C: Compatibility and performance: Web Platform Tests (record pass/fail and baseline), real-world sites (search, email, office, Hi5Central), media playback, WebRTC remote viewer, responsiveness, memory, crash and sandbox testing. Explicitly record unsupported features.
D: Browser product features: privacy controls, private browsing, multiple profiles, password storage only with protected vault, autofill, accessibility, printing/PDF, keyboard shortcuts, session restoration, extension compatibility feasibility, updates. Sync opt-in and separately designed; no implicit credential export. DRM/proprietary codecs subject to licensing and availability.
E: Daily-driver gate: stable supported sites, robust secure browsing, independent security review, reliable update process, WPT and performance comparisons on identical hardware. No claim of Chrome/Firefox parity until evidence supports it.

## Work tracking
Separate browser development and 7-Zip GUI qualification; merge shared platform changes through CI without weakening archive gates. First implement secure platform primitives and local engine feasibility rather than putting a JavaScript interpreter or arbitrary pages into kernel mode. Browser plan only: engine integration and runtime are not implemented by this document.
