#include <string.h>

void *memcpy(void *dst, const void *src, size_t n) {
    unsigned char *d = dst;
    const unsigned char *s = src;

    for (size_t i = 0; i < n; i++)
        d[i] = s[i];

    return dst;
}

void *memmove(void *dst, const void *src, size_t n) {
    unsigned char *d = dst;
    const unsigned char *s = src;

    if (d < s) {
        for (size_t i = 0; i < n; i++)
            d[i] = s[i];
    } else if (d > s) {
        for (size_t i = n; i > 0; i--)
            d[i - 1] = s[i - 1];
    }

    return dst;
}

void *memset(void *dst, int c, size_t n) {
    unsigned char *d = dst;

    for (size_t i = 0; i < n; i++)
        d[i] = (unsigned char)c;

    return dst;
}

int memcmp(const void *a, const void *b, size_t n) {
    const unsigned char *x = a;
    const unsigned char *y = b;

    for (size_t i = 0; i < n; i++) {
        if (x[i] != y[i])
            return x[i] < y[i] ? -1 : 1;
    }

    return 0;
}

size_t strlen(const char *s) {
    size_t n = 0;

    while (s[n])
        n++;

    return n;
}

size_t strnlen(const char *s, size_t max) {
    size_t n = 0;

    while (n < max && s[n])
        n++;

    return n;
}

int strcmp(const char *a, const char *b) {
    while (*a && *a == *b) {
        a++;
        b++;
    }

    return (unsigned char)*a - (unsigned char)*b;
}

int strncmp(const char *a, const char *b, size_t n) {
    for (size_t i = 0; i < n; i++) {
        unsigned char x = a[i];
        unsigned char y = b[i];

        if (x != y)
            return x - y;

        if (!x)
            return 0;
    }

    return 0;
}

char *strcpy(char *dst, const char *src) {
    char *out = dst;

    while ((*dst++ = *src++));

    return out;
}

char *strncpy(char *dst, const char *src, size_t n) {
    size_t i = 0;

    for (; i < n && src[i]; i++)
        dst[i] = src[i];

    for (; i < n; i++)
        dst[i] = 0;

    return dst;
}

char *strchr(const char *s, int c) {
    while (*s) {
        if (*s == c)
            return (char *)s;

        s++;
    }

    return c == 0 ? (char *)s : NULL;
}
