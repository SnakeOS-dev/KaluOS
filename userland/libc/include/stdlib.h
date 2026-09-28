#pragma once

#include <stddef.h>

void *malloc(size_t size);
void *calloc(size_t count, size_t size);
void *realloc(void *ptr, size_t size);
void free(void *ptr);

void exit(int code) __attribute__((noreturn));

int atoi(const char *s);
