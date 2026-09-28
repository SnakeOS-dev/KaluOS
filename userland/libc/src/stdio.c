#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <stdarg.h>
#include <stdint.h>
#include <kalu/syscall.h>

static FILE stdin_file = { 0 };
static FILE stdout_file = { 1 };
static FILE stderr_file = { 2 };

FILE *stdin = &stdin_file;
FILE *stdout = &stdout_file;
FILE *stderr = &stderr_file;

int putchar(int c) {
    unsigned char ch = c;

    return write(
        1,
        &ch,
        1
    ) == 1
        ? c
        : -1;
}

int puts(const char *s) {
    size_t len = strlen(s);

    if (write(1, s, len) != (long)len)
        return -1;

    if (write(1, "\n", 1) != 1)
        return -1;

    return 0;
}

int getchar(void) {
    unsigned char c;

    if (read(0, &c, 1) != 1)
        return -1;

    return c;
}

static int print_unsigned(
    unsigned long value,
    unsigned base,
    int prefix
) {
    char buffer[32];
    char *digits =
        "0123456789abcdef";

    int i = 0;
    int count = 0;

    if (prefix) {
        putchar('0');
        putchar('x');
        count += 2;
    }

    if (!value) {
        putchar('0');
        return count + 1;
    }

    while (value) {
        buffer[i++] =
            digits[value % base];

        value /= base;
    }

    while (i) {
        putchar(buffer[--i]);
        count++;
    }

    return count;
}

static int print_signed(long value) {
    int count = 0;

    if (value < 0) {
        putchar('-');
        count++;

        return count
            + print_unsigned(
                (unsigned long)(-value),
                10,
                0
            );
    }

    return print_unsigned(
        value,
        10,
        0
    );
}

int printf(const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);

    int count = 0;

    while (*fmt) {
        if (*fmt != '%') {
            putchar(*fmt++);
            count++;
            continue;
        }

        fmt++;

        if (!*fmt)
            break;

        switch (*fmt) {
            case '%':
                putchar('%');
                count++;
                break;

            case 'c': {
                int c =
                    va_arg(args, int);

                putchar(c);
                count++;
                break;
            }

            case 's': {
                const char *s =
                    va_arg(
                        args,
                        const char *
                    );

                if (!s)
                    s = "(null)";

                size_t len =
                    strlen(s);

                write(1, s, len);
                count += len;
                break;
            }

            case 'd':
            case 'i':
                count += print_signed(
                    va_arg(args, int)
                );
                break;

            case 'u':
                count += print_unsigned(
                    va_arg(args, unsigned),
                    10,
                    0
                );
                break;

            case 'x':
                count += print_unsigned(
                    va_arg(args, unsigned),
                    16,
                    0
                );
                break;

            case 'p':
                count += print_unsigned(
                    (uintptr_t)
                    va_arg(args, void *),
                    16,
                    1
                );
                break;

            case 'l':
                fmt++;

                if (*fmt == 'u') {
                    count += print_unsigned(
                        va_arg(
                            args,
                            unsigned long
                        ),
                        10,
                        0
                    );
                } else if (*fmt == 'd') {
                    count += print_signed(
                        va_arg(args, long)
                    );
                } else if (*fmt == 'x') {
                    count += print_unsigned(
                        va_arg(
                            args,
                            unsigned long
                        ),
                        16,
                        0
                    );
                }

                break;

            default:
                putchar('%');
                putchar(*fmt);
                count += 2;
                break;
        }

        fmt++;
    }

    va_end(args);

    return count;
}

FILE *fopen(const char *path, const char *mode) {
    int flags = 0;

    if (!strcmp(mode, "r")) {
        flags = O_READ;
    } else if (!strcmp(mode, "w")) {
        flags =
            O_WRITE
            | O_CREATE
            | O_TRUNC;
    } else if (!strcmp(mode, "a")) {
        flags =
            O_WRITE
            | O_CREATE;
    } else if (!strcmp(mode, "r+")) {
        flags =
            O_READ
            | O_WRITE;
    } else if (!strcmp(mode, "w+")) {
        flags =
            O_READ
            | O_WRITE
            | O_CREATE
            | O_TRUNC;
    } else {
        return NULL;
    }

    int fd = open(path, flags);

    if (fd < 0)
        return NULL;

    FILE *file = malloc(
        sizeof(FILE)
    );

    if (!file) {
        close(fd);
        return NULL;
    }

    file->fd = fd;

    return file;
}

size_t fread(
    void *ptr,
    size_t size,
    size_t count,
    FILE *file
) {
    if (!size || !count)
        return 0;

    size_t total =
        size * count;

    long result =
        read(
            file->fd,
            ptr,
            total
        );

    if (result <= 0)
        return 0;

    return result / size;
}

size_t fwrite(
    const void *ptr,
    size_t size,
    size_t count,
    FILE *file
) {
    if (!size || !count)
        return 0;

    size_t total =
        size * count;

    long result =
        write(
            file->fd,
            ptr,
            total
        );

    if (result <= 0)
        return 0;

    return result / size;
}

int fclose(FILE *file) {
    if (!file)
        return -1;

    if (
        file == stdin ||
        file == stdout ||
        file == stderr
    )
        return 0;

    int result =
        close(file->fd);

    free(file);

    return result;
}
