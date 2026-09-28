#include <unistd.h>
#include <string.h>
#include <stdint.h>
#include <kalu/syscall.h>

ssize_t read(int fd, void *buf, size_t size) {
    return (ssize_t)__syscall3(
        SYS_READ,
        fd,
        (uint64_t)(uintptr_t)buf,
        size
    );
}

ssize_t write(int fd, const void *buf, size_t size) {
    return (ssize_t)__syscall3(
        SYS_WRITE,
        fd,
        (uint64_t)(uintptr_t)buf,
        size
    );
}

int open(const char *path, int flags) {
    return (int)__syscall3(
        SYS_OPEN,
        (uint64_t)(uintptr_t)path,
        strlen(path),
        flags
    );
}

int close(int fd) {
    return (int)__syscall1(
        SYS_CLOSE,
        fd
    );
}

int chdir(const char *path) {
    return (int)__syscall2(
        SYS_CHDIR,
        (uint64_t)(uintptr_t)path,
        strlen(path)
    );
}

int unlink(const char *path) {
    return (int)__syscall2(
        SYS_REMOVE,
        (uint64_t)(uintptr_t)path,
        strlen(path)
    );
}

int mkdir(const char *path) {
    return (int)__syscall2(
        SYS_MKDIR,
        (uint64_t)(uintptr_t)path,
        strlen(path)
    );
}

int rmdir(const char *path) {
    return (int)__syscall2(
        SYS_RMDIR,
        (uint64_t)(uintptr_t)path,
        strlen(path)
    );
}

int spawn(const char *path, int priority) {
    return (int)__syscall3(
        SYS_SPAWN,
        (uint64_t)(uintptr_t)path,
        strlen(path),
        priority
    );
}

int exec(const char *path) {
    return (int)__syscall2(
        SYS_EXEC,
        (uint64_t)(uintptr_t)path,
        strlen(path)
    );
}

int test_file(const char *path) {
    return (int)__syscall2(
        SYS_TEST_FILE,
        (uint64_t)(uintptr_t)path,
        strlen(path)
    );
}

int getc_raw(void) {
    return (int)__syscall0(
        SYS_GETC
    );
}

uint64_t ticks(void) {
    return __syscall0(
        SYS_TICKS
    );
}

void yield(void) {
    __syscall0(
        SYS_YIELD
    );
}

int sysname(char *buffer, size_t size) {
    return (int)__syscall2(
        SYS_SYSNAME,
        (uint64_t)(uintptr_t)buffer,
        size
    );
}
#include <dirent.h>

int readdir(int fd, dirent_t *entry) {
    return (int)__syscall2(
        SYS_READDIR,
        fd,
        (uint64_t)(uintptr_t)entry
    );
}
