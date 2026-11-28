# Hi5OS product and engineering roadmap
Goal: a viable Windows replacement for everyday business workloads with native Hi5Central RMM and ITSM from first-run setup. Preserve offline/local operation; management enabled only through authorised tenant enrolment. Continue cross-platform Hi5Central management.
## Ordered delivery
1. Hi5Office (Word, Sheets, Slides, PDF): native versioned formats and documented interoperability; qualification in external office applications.
2. Platform foundations: general filesystem and permissions, userspace process isolation, scheduling, IPC, drivers, Unicode, accessibility, graphics, networking, audio, printing, installer, signed updates and recovery. Prioritise dependencies before adding more kernel-resident applications.
3. Hi5Central native management: define a versioned device API and isolated service, tenant binding, revocable per-device credentials, signed scoped commands, role checks and audit records. Initial read-only device health and inventory before privileged actions. Use isolated QA tenant on the primary *.hi5central.com environment; no EU test environment or changes to production services without explicit approval.
4. Hi5Support: tickets, service requests, knowledge base, notifications, live chat, self-service catalogue and consent-aware support; links to device records.
5. Hi5Identity/Hi5Manage/Hi5Deploy: local and enterprise identity, offline sign-in, enrolment, policy, compliance, deployment and recovery. Never embed tenant secrets in images.
6. Hi5Store: signed .h5pkg, dependencies, permissions, updates, uninstall and rollback.
7. Hi5Browser: retain Build 94 baseline; evaluate mature rendering engine once process/runtime APIs exist. Gate on real Google search and BBC page behaviour; no credential use until isolation and storage security qualified.
8. Collaboration and utilities: Hi5Mail, Calendar, Chat, Meet, Cloud, Notes, Photos, Media, Capture, Calculator, Archive, Backup, Remote and Task Manager.
9. Security: verified boot, disk encryption, firewall, protected credentials, sandboxing, endpoint detection, audit and compliance. Security work spans every stage, not a late add-on.
10. Hi5Win compatibility: EXE/MSI, Win32, registry, DLLs and qualified business applications. Preserve 7-Zip regression gates. Physical hardware and pilot qualification with workload-specific acceptance.
## Release discipline
Build numbers are milestones not promises; existing mandatory regression tests stay intact. Preserve data disks and versioned formats. No production Hi5Central changes during OS-only development. Record source commit, actual CI status, manual evidence, limitations and rollback baseline for each build. Home/Business/Enterprise editions are proposed, not yet implemented.

## GUI and platform milestones after CI #101
102: responsive shell, persistent appearance, frost/damage presentation and navigation; 103: qualified VMware SVGA mode setting and FIFO; 104: independent window surfaces and compositor; 105: app shell refinement; 106–110: filesystem, runtime, multitasking, signed update and packaging/security. These are qualification goals, not completed features. Preserve existing data formats and VM upgrades; avoid unqualified device writes.

103 implementation: guarded active SVGA II native mode preview with timed rollback and local network diagnostics; full FIFO/GPU support remains unqualified. Retain firmware fallback and VMware system-only package.

104: retained interactive foreground surface, cached backdrop, damage-region composition and paced drag (up to 50 Hz); persistent app surfaces and full window resizing/animations remain future. 105 native application refinement, then Hi5FS/runtime/updates.
