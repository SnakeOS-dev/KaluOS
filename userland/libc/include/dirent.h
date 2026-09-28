#pragma once

#include <stdint.h>

typedef struct {
    uint32_t inode;
    uint8_t type;
    uint8_t name_len;
    uint16_t reserved;
    char name[256];
} dirent_t;

int readdir(int fd, dirent_t *entry);
