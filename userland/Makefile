CC = clang
LD = ld.lld
AR = llvm-ar

CFLAGS = \
        -target x86_64-none-elf \
        -ffreestanding \
        -fno-stack-protector \
        -fno-pic \
        -mno-mmx \
        -mno-sse \
        -mno-sse2 \
        -mno-red-zone \
        -mcmodel=small \
        -O2 \
        -Wall \
        -Wextra \
        -Ilibc/include

LIBC_SRC = \
        libc/src/syscall.c \
        libc/src/errno.c \
        libc/src/string.c \
        libc/src/malloc.c \
        libc/src/unistd.c \
        libc/src/stdio.c

LIBC_OBJ  = $(LIBC_SRC:.c=.o)
CRT0      = libc/src/crt0.o
PROGRAMS  = init kalush kalubox
ELFS      = $(addsuffix .elf,$(PROGRAMS))

all: $(ELFS)

libc.a: $(LIBC_OBJ)
	$(AR) rcs $@ $^

$(CRT0): libc/src/crt0.S
	$(CC) $(CFLAGS) -c $< -o $@

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

%.elf: $(CRT0) programs/%/main.o libc.a
	$(LD) -T linker.ld -o $@ $(CRT0) programs/$*/main.o libc.a

clean:
	rm -f $(LIBC_OBJ) $(CRT0) programs/*/main.o libc.a $(ELFS)

.PHONY: all clean
