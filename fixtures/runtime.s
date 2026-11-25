# x64 Windows API calling convention, built independently by clang/lld.
.text
.globl hi5_entry
hi5_entry:
    subq $40, %rsp
    callq *__imp_GetCurrentProcessId(%rip)
    cmpl $1, %eax
    jne failed
    callq *__imp_GetLastError(%rip)
    testl %eax, %eax
    jne failed
    movl $1234, %ecx
    callq *__imp_SetLastError(%rip)
    callq *__imp_GetLastError(%rip)
    cmpl $1234, %eax
    jne failed
    movl $42, %ecx
    jmp finish
failed:
    movl $99, %ecx
finish:
    callq *__imp_ExitProcess(%rip)
    ud2
