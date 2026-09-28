.global _start
.extern main
.extern exit

.section .text

_start:
    xor %rbp, %rbp
    and $-16, %rsp
    sub $8, %rsp
    call main
    mov %eax, %edi
    call exit

1:
    hlt
    jmp 1b
