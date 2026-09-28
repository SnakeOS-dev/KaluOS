#pragma once

#include <stddef.h>
#include <stdint.h>
#define O_READ 1
#define O_WRITE 2
#define O_CREATE 4
#define O_TRUNC 8
typedef long ssize_t;

ssize_t read(int fd, void *buf, size_t size);
ssize_t write(int fd, const void *buf, size_t size);

int open(const char *path, int flags);
int close(int fd);

int chdir(const char *path);
int unlink(const char *path);
int mkdir(const char *path);
int rmdir(const char *path);

int spawn(const char *path, int priority);
int exec(const char *path);

int test_file(const char *path);

int getc_raw(void);

uint64_t ticks(void);
void yield(void);

int sysname(char *buffer, size_t size);
