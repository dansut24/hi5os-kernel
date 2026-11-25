# Windows x64 console fixture. No native Hi5OS calls; imported Win32 ABI only.
.text
.globl hi5_entry
hi5_entry:
    subq $72, %rsp
    movq %gs:0x60, %rax
    testq %rax, %rax
    jz failed
    movq 0x10(%rax), %rax
    movabsq $0x100000000, %rcx
    cmpq %rcx, %rax
    jne failed
    callq *__imp_GetCommandLineW(%rip)
    testq %rax, %rax
    jz failed
    cmpw $104, (%rax)
    jne failed
    cmpw $45, 38(%rax)
    jne failed
    cmpw $104, 40(%rax)
    jne failed
    cmpw $0, 42(%rax)
    jne failed
    movl $-11, %ecx
    callq *__imp_GetStdHandle(%rip)
    cmpq $1, %rax
    jne failed
    movq %rax, %rcx
    leaq message(%rip), %rdx
    movl $27, %r8d
    leaq 48(%rsp), %r9
    movl $0, 48(%rsp)
    movq $0, 32(%rsp)
    callq *__imp_WriteFile(%rip)
    cmpl $1, %eax
    jne failed
    cmpl $27, 48(%rsp)
    jne failed
    # Check invalid pointer rejected, without triggering a kernel fault.
    movl $1, %ecx
    xorq %rdx, %rdx
    movl $1, %r8d
    leaq 48(%rsp), %r9
    movq $0, 32(%rsp)
    callq *__imp_WriteFile(%rip)
    testl %eax, %eax
    jne failed
    movl $42, %ecx
    jmp finish
failed:
    movl $99, %ecx
finish:
    callq *__imp_ExitProcess(%rip)
    ud2
.section .rdata,"dr"
message:
    .ascii "Hello from Windows console!"
