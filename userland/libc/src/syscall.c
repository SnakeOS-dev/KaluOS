#include <kalu/syscall.h>

uint64_t __syscall0(uint64_t n) {
    uint64_t r;

    __asm__ volatile(
        "int $0x80"
        : "=a"(r)
        : "a"(n)
        : "memory"
    );

    return r;
}

uint64_t __syscall1(uint64_t n, uint64_t a) {
    uint64_t r;

    __asm__ volatile(
        "int $0x80"
        : "=a"(r)
        : "a"(n), "D"(a)
        : "memory"
    );

    return r;
}

uint64_t __syscall2(uint64_t n, uint64_t a, uint64_t b) {
    uint64_t r;

    __asm__ volatile(
        "int $0x80"
        : "=a"(r)
        : "a"(n), "D"(a), "S"(b)
        : "memory"
    );

    return r;
}

uint64_t __syscall3(uint64_t n, uint64_t a, uint64_t b, uint64_t c) {
    uint64_t r;

    __asm__ volatile(
        "int $0x80"
        : "=a"(r)
        : "a"(n), "D"(a), "S"(b), "d"(c)
        : "memory"
    );

    return r;
}

uint64_t __syscall4(uint64_t n, uint64_t a, uint64_t b, uint64_t c, uint64_t d) {
    register uint64_t r10 __asm__("r10") = d;
    uint64_t r;

    __asm__ volatile(
        "int $0x80"
        : "=a"(r)
        : "a"(n), "D"(a), "S"(b), "d"(c), "r"(r10)
        : "memory"
    );

    return r;
}

uint64_t __syscall5(uint64_t n, uint64_t a, uint64_t b, uint64_t c, uint64_t d, uint64_t e) {
    register uint64_t r10 __asm__("r10") = d;
    register uint64_t r8 __asm__("r8") = e;
    uint64_t r;

    __asm__ volatile(
        "int $0x80"
        : "=a"(r)
        : "a"(n), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8)
        : "memory"
    );

    return r;
}

uint64_t __syscall6(uint64_t n, uint64_t a, uint64_t b, uint64_t c, uint64_t d, uint64_t e, uint64_t f) {
    register uint64_t r10 __asm__("r10") = d;
    register uint64_t r8 __asm__("r8") = e;
    register uint64_t r9 __asm__("r9") = f;
    uint64_t r;

    __asm__ volatile(
        "int $0x80"
        : "=a"(r)
        : "a"(n), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8), "r"(r9)
        : "memory"
    );

    return r;
}
