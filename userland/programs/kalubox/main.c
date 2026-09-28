#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <dirent.h>

static int cmd_ls(
    int argc,
    char **argv
) {
    const char *path =
        argc >= 2
            ? argv[1]
            : ".";

    int fd =
        open(
            path,
            O_READ
        );

    if (fd < 0) {
        printf(
            "ls: cannot open %s\n",
            path
        );

        return 1;
    }

    dirent_t entry;

    while (
        readdir(
            fd,
            &entry
        ) > 0
    ) {
        if (
            entry.name_len >= 255
        ) {
            entry.name_len = 255;
        }

        entry.name[
            entry.name_len
        ] = 0;

        if (
            !strcmp(
                entry.name,
                "."
            )
            || !strcmp(
                entry.name,
                ".."
            )
        ) {
            continue;
        }

        printf(
            "%s\n",
            entry.name
        );
    }

    close(fd);

    return 0;
}

static int cmd_cat(
    int argc,
    char **argv
) {
    if (argc < 2) {
        printf(
            "usage: cat FILE\n"
        );

        return 1;
    }

    int fd =
        open(
            argv[1],
            O_READ
        );

    if (fd < 0) {
        printf(
            "cat: cannot open %s\n",
            argv[1]
        );

        return 1;
    }

    char buffer[512];

    for (;;) {
        long count =
            read(
                fd,
                buffer,
                sizeof(buffer)
            );

        if (count <= 0)
            break;

        write(
            1,
            buffer,
            count
        );
    }

    close(fd);

    return 0;
}

static int cmd_echo(
    int argc,
    char **argv
) {
    for (
        int i = 1;
        i < argc;
        i++
    ) {
        if (i > 1)
            putchar(' ');

        write(
            1,
            argv[i],
            strlen(argv[i])
        );
    }

    putchar('\n');

    return 0;
}

static int cmd_mkdir(
    int argc,
    char **argv
) {
    if (argc < 2)
        return 1;

    return mkdir(
        argv[1]
    ) < 0;
}

static int cmd_rm(
    int argc,
    char **argv
) {
    if (argc < 2)
        return 1;

    return unlink(
        argv[1]
    ) < 0;
}

static int cmd_rmdir(
    int argc,
    char **argv
) {
    if (argc < 2)
        return 1;

    return rmdir(
        argv[1]
    ) < 0;
}

static int cmd_touch(
    int argc,
    char **argv
) {
    if (argc < 2)
        return 1;

    int fd =
        open(
            argv[1],
            O_WRITE
                | O_CREATE
        );

    if (fd < 0)
        return 1;

    close(fd);

    return 0;
}

static int cmd_ticks(
    int argc,
    char **argv
) {
    (void)argc;
    (void)argv;

    printf(
        "%lu\n",
        ticks()
    );

    return 0;
}

static int cmd_sysname(
    int argc,
    char **argv
) {
    (void)argc;
    (void)argv;

    char name[64];

    int count =
        sysname(
            name,
            sizeof(name) - 1
        );

    if (count < 0)
        return 1;

    name[count] = 0;

    printf(
        "%s\n",
        name
    );

    return 0;
}

static int cmd_sleep(
    int argc,
    char **argv
) {
    if (argc < 2) {
        printf(
            "usage: sleep MS\n"
        );

        return 1;
    }

    int ms =
        atoi(
            argv[1]
        );

    if (ms < 0)
        return 1;

    sleep_ms(
        (uint64_t)ms
    );

    return 0;
}

static int cmd_clear(
    int argc,
    char **argv
) {
    (void)argc;
    (void)argv;

    write(
        1,
        "\033[2J\033[H",
        7
    );

    return 0;
}

static int cmd_help(
    int argc,
    char **argv
) {
    (void)argc;
    (void)argv;

    printf(
        "KaluBox\n"
        "ls [DIR]\n"
        "cat FILE\n"
        "echo TEXT...\n"
        "mkdir DIR\n"
        "rm FILE\n"
        "rmdir DIR\n"
        "touch FILE\n"
        "ticks\n"
        "sysname\n"
        "sleep MS\n"
        "clear\n"
        "help\n"
    );

    return 0;
}

typedef int (*command_fn)(
    int,
    char **
);

typedef struct {
    const char *name;
    command_fn fn;
} command_t;

static command_t commands[] = {
    {
        "ls",
        cmd_ls
    },
    {
        "cat",
        cmd_cat
    },
    {
        "echo",
        cmd_echo
    },
    {
        "mkdir",
        cmd_mkdir
    },
    {
        "rm",
        cmd_rm
    },
    {
        "rmdir",
        cmd_rmdir
    },
    {
        "touch",
        cmd_touch
    },
    {
        "ticks",
        cmd_ticks
    },
    {
        "sysname",
        cmd_sysname
    },
    {
        "sleep",
        cmd_sleep
    },
    {
        "clear",
        cmd_clear
    },
    {
        "help",
        cmd_help
    }
};

int main(
    int argc,
    char **argv
) {
    if (argc < 2) {
        return cmd_help(
            argc,
            argv
        );
    }

    size_t count =
        sizeof(commands)
        / sizeof(commands[0]);

    for (
        size_t i = 0;
        i < count;
        i++
    ) {
        if (!strcmp(
            argv[1],
            commands[i].name
        )) {
            return commands[i].fn(
                argc - 1,
                argv + 1
            );
        }
    }

    printf(
        "kalubox: unknown command: %s\n",
        argv[1]
    );

    return 127;
}
