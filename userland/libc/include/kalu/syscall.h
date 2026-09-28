#pragma once
#include <stdint.h>
typedef struct {
    const char *ptr;
    uint64_t len;
} kalu_arg_t;
#define SYS_TEXT 1
#define SYS_READ 2
#define SYS_WRITE 3
#define SYS_READDIR 4
#define SYS_CHDIR 5
#define SYS_REMOVE 6
#define SYS_SPAWN 7
#define SYS_GETC 8
#define SYS_MAP 9
#define SYS_UNMAP 10
#define SYS_YIELD 11
#define SYS_TICKS 12
#define SYS_SYSNAME 13
#define SYS_EXEC 14
#define SYS_OPEN 15
#define SYS_CLOSE 16
#define SYS_MKDIR 17
#define SYS_RMDIR 18
#define SYS_TEST_FILE 19
#define SYS_EXIT 20
#define SYS_WAIT 21
#define SYS_SLEEP 22
#define O_READ 1
#define O_WRITE 2
#define O_CREATE 4
#define O_TRUNC 8
#define MAP_WRITE 1
#define MAP_EXEC 2
uint64_t __syscall0(uint64_t n);
uint64_t __syscall1(uint64_t n, uint64_t a);
uint64_t __syscall2(uint64_t n, uint64_t a, uint64_t b);
uint64_t __syscall3(uint64_t n, uint64_t a, uint64_t b, uint64_t c);
uint64_t __syscall4(uint64_t n, uint64_t a, uint64_t b, uint64_t c, uint64_t d);
uint64_t __syscall5(uint64_t n, uint64_t a, uint64_t b, uint64_t c, uint64_t d, uint64_t e);
uint64_t __syscall6(uint64_t n, uint64_t a, uint64_t b, uint64_t c, uint64_t d, uint64_t e, uint64_t f);
