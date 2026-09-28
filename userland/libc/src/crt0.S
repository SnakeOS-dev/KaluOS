.global _start
.extern main
.extern exit

.section .text

_start:
    xor %rbp, %rbp

    mov (%rsp), %rdi
    lea 8(%rsp), %rsi

    and $-16, %rsp
    sub $8, %rsp

    call main

    mov %eax, %edi
    call exit

1:
    mov $11, %rax
    int $0x80
    jmp 1b
