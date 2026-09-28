#include <unistd.h>
#include <string.h>
#include <stdint.h>
#include <kalu/syscall.h>
#include <stdlib.h>

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

int spawn(
    const char *path,
    int argc,
    char **argv,
    int priority
) {
    if (argc < 0)
        return -1;

    kalu_arg_t *args =
        NULL;

    if (argc > 0) {
        args =
            malloc(
                sizeof(kalu_arg_t)
                * argc
            );

        if (!args)
            return -1;

        for (
            int i = 0;
            i < argc;
            i++
        ) {
            args[i].ptr =
                argv[i];

            args[i].len =
                strlen(argv[i]);
        }
    }

    uint64_t result =
        __syscall5(
            SYS_SPAWN,
            (uint64_t)
                (uintptr_t)path,
            strlen(path),
            argc,
            (uint64_t)
                (uintptr_t)args,
            priority
        );

    free(args);

    return (int)result;
}
int exec(
    const char *path,
    int argc,
    char **argv
) {
    kalu_arg_t *args =
        NULL;

    if (argc > 0) {
        args =
            malloc(
                sizeof(kalu_arg_t)
                * argc
            );

        if (!args)
            return -1;

        for (
            int i = 0;
            i < argc;
            i++
        ) {
            args[i].ptr =
                argv[i];

            args[i].len =
                strlen(argv[i]);
        }
    }

    uint64_t result =
        __syscall4(
            SYS_EXEC,
            (uint64_t)
                (uintptr_t)path,
            strlen(path),
            argc,
            (uint64_t)
                (uintptr_t)args
        );

    free(args);

    return (int)result;
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
int wait(int pid, int *status) {
    for (;;) {
        int result =
            (int)__syscall2(
                SYS_WAIT,
                (uint64_t)pid,
                (uint64_t)(uintptr_t)status
            );

        if (result >= 0)
            return result;

        if (result != -9)
            return result;

        yield();
    }
}

void sleep_ticks(uint64_t ticks_count) {
    __syscall1(
        SYS_SLEEP,
        ticks_count
    );
}

void sleep_ms(uint64_t ms) {
    uint64_t count =
        (ms + 9) / 10;

    if (ms && count == 0)
        count = 1;

    sleep_ticks(count);
}

void sleep(unsigned int seconds) {
    sleep_ticks(
        (uint64_t)seconds * 100
    );
}
