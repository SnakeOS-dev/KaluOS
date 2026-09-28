#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <kalu/syscall.h>

typedef struct block {
    size_t size;
    int free;
    struct block *next;
} block_t;

static block_t *head;

static size_t align16(size_t n) {
    return (n + 15) & ~(size_t)15;
}

static block_t *request_region(size_t size) {
    size_t total = sizeof(block_t) + size;
    size_t pages = (total + 4095) & ~(size_t)4095;

    uint64_t result = __syscall3(
        SYS_MAP,
        0,
        pages,
        MAP_WRITE
    );

    if ((int64_t)result < 0)
        return NULL;

    block_t *block = (block_t *)(uintptr_t)result;

    block->size = pages - sizeof(block_t);
    block->free = 1;
    block->next = NULL;

    return block;
}

static void split(block_t *block, size_t size) {
    if (block->size < size + sizeof(block_t) + 16)
        return;

    block_t *next = (block_t *)(
        (unsigned char *)(block + 1) + size
    );

    next->size =
        block->size - size - sizeof(block_t);

    next->free = 1;
    next->next = block->next;

    block->size = size;
    block->next = next;
}

static void merge(void) {
    block_t *block = head;

    while (block && block->next) {
        unsigned char *end =
            (unsigned char *)(block + 1)
            + block->size;

        if (
            block->free &&
            block->next->free &&
            end == (unsigned char *)block->next
        ) {
            block->size +=
                sizeof(block_t)
                + block->next->size;

            block->next =
                block->next->next;
        } else {
            block = block->next;
        }
    }
}

void *malloc(size_t size) {
    if (!size)
        return NULL;

    size = align16(size);

    block_t *block = head;
    block_t *last = NULL;

    while (block) {
        if (block->free && block->size >= size) {
            split(block, size);
            block->free = 0;
            return block + 1;
        }

        last = block;
        block = block->next;
    }

    block = request_region(size);

    if (!block)
        return NULL;

    if (last)
        last->next = block;
    else
        head = block;

    split(block, size);
    block->free = 0;

    return block + 1;
}

void free(void *ptr) {
    if (!ptr)
        return;

    block_t *block =
        (block_t *)ptr - 1;

    block->free = 1;

    merge();
}

void *calloc(size_t count, size_t size) {
    if (count && size > (size_t)-1 / count)
        return NULL;

    size_t total = count * size;

    void *ptr = malloc(total);

    if (ptr)
        memset(ptr, 0, total);

    return ptr;
}

void *realloc(void *ptr, size_t size) {
    if (!ptr)
        return malloc(size);

    if (!size) {
        free(ptr);
        return NULL;
    }

    block_t *block =
        (block_t *)ptr - 1;

    if (block->size >= size)
        return ptr;

    void *next = malloc(size);

    if (!next)
        return NULL;

    memcpy(next, ptr, block->size);

    free(ptr);

    return next;
}

int atoi(const char *s) {
    int sign = 1;
    int value = 0;

    while (*s == ' ' || *s == '\t')
        s++;

    if (*s == '-') {
        sign = -1;
        s++;
    } else if (*s == '+') {
        s++;
    }

    while (*s >= '0' && *s <= '9') {
        value =
            value * 10
            + (*s - '0');

        s++;
    }

    return value * sign;
}

void exit(int code) {
    __syscall1(
        SYS_EXIT,
        (uint64_t)(int64_t)code
    );

    for (;;)
        __asm__ volatile("hlt");
}
