#pragma once

#include <stddef.h>

typedef struct {
    int fd;
} FILE;

extern FILE *stdin;
extern FILE *stdout;
extern FILE *stderr;

int putchar(int c);
int puts(const char *s);
int getchar(void);

int printf(const char *fmt, ...);

FILE *fopen(const char *path, const char *mode);
size_t fread(void *ptr, size_t size, size_t count, FILE *file);
size_t fwrite(const void *ptr, size_t size, size_t count, FILE *file);
int fclose(FILE *file);
