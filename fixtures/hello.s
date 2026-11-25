# Tiny Windows x64 console PE. Standard Win64 ABI, no CRT, one Win32 import.
.text
.globl hi5_entry
hi5_entry:
    subq $40, %rsp
    movl $42, %ecx
    callq *__imp_ExitProcess(%rip)
    ud2
