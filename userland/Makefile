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

LIBC_OBJ = $(LIBC_SRC:.c=.o)

all: init.elf

libc.a: $(LIBC_OBJ)
	$(AR) rcs $@ $(LIBC_OBJ)

libc/src/crt0.o: libc/src/crt0.S
	$(CC) $(CFLAGS) -c $< -o $@

programs/init/main.o: programs/init/main.c
	$(CC) $(CFLAGS) -c $< -o $@

init.elf: libc.a libc/src/crt0.o programs/init/main.o
	$(LD) \
		-T linker.ld \
		-o $@ \
		libc/src/crt0.o \
		programs/init/main.o \
		libc.a

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f \
		$(LIBC_OBJ) \
		libc/src/crt0.o \
		programs/init/main.o \
		libc.a \
		init.elf
