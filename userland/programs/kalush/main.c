#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define LINE_SIZE 512
#define MAX_ARGS 32

static int read_line(
    char *buffer,
    int size
) {
    int length = 0;

    for (;;) {
        int c =
            getchar();

        if (c < 0) {
            yield();
            continue;
        }

        if (c == '\r')
            continue;

        if (c == '\n') {
            putchar('\n');
            buffer[length] = 0;
            return length;
        }

        if (
            c == 8
            || c == 127
        ) {
            if (length > 0) {
                length--;

                write(
                    1,
                    "\b \b",
                    3
                );
            }

            continue;
        }

        if (
            c >= 32
            && c <= 126
            && length < size - 1
        ) {
            buffer[length++] =
                (char)c;

            putchar(c);
        }
    }
}

static int parse_line(
    char *line,
    char **argv,
    int max
) {
    int argc = 0;
    char *p = line;

    while (*p) {
        while (
            *p == ' '
            || *p == '\t'
        ) {
            p++;
        }

        if (!*p)
            break;

        if (argc >= max)
            break;

        if (
            *p == '"'
            || *p == '\''
        ) {
            char quote = *p++;

            argv[argc++] = p;

            while (
                *p
                && *p != quote
            ) {
                p++;
            }

            if (*p)
                *p++ = 0;

            continue;
        }

        argv[argc++] = p;

        while (
            *p
            && *p != ' '
            && *p != '\t'
        ) {
            p++;
        }

        if (*p)
            *p++ = 0;
    }

    return argc;
}

static int is_box_command(
    const char *name
) {
    static const char *commands[] = {
        "ls",
        "cat",
        "echo",
        "mkdir",
        "rm",
        "rmdir",
        "touch",
        "ticks",
        "sysname",
        "sleep",
        "clear",
        "help"
    };

    size_t count =
        sizeof(commands)
        / sizeof(commands[0]);

    for (
        size_t i = 0;
        i < count;
        i++
    ) {
        if (!strcmp(
            name,
            commands[i]
        )) {
            return 1;
        }
    }

    return 0;
}

static int run(
    const char *path,
    int argc,
    char **argv
) {
    int pid =
        spawn(
            path,
            argc,
            argv,
            5
        );

    if (pid < 0) {
        printf(
            "kalush: failed to run %s\n",
            path
        );

        return 127;
    }

    int status = 0;

    wait(
        pid,
        &status
    );

    return status;
}

static int run_box(
    int argc,
    char **argv
) {
    char *args[MAX_ARGS + 1];

    args[0] =
        "/bin/kalubox";

    for (
        int i = 0;
        i < argc;
        i++
    ) {
        args[i + 1] =
            argv[i];
    }

    return run(
        "/bin/kalubox",
        argc + 1,
        args
    );
}

int main(
    int argc,
    char **argv
) {
    (void)argc;
    (void)argv;

    printf(
        "KaluSH\n"
    );

    char line[LINE_SIZE];
    char *args[MAX_ARGS];

    for (;;) {
        printf(
            "kalu@localhost# "
        );

        int length =
            read_line(
                line,
                sizeof(line)
            );

        if (length <= 0)
            continue;

        int count =
            parse_line(
                line,
                args,
                MAX_ARGS
            );

        if (count == 0)
            continue;

        if (!strcmp(
            args[0],
            "cd"
        )) {
            const char *path =
                count >= 2
                    ? args[1]
                    : "/";

            if (chdir(path) < 0) {
                printf(
                    "cd: %s: failed\n",
                    path
                );
            }

            continue;
        }

        if (!strcmp(
            args[0],
            "exit"
        )) {
            int code =
                count >= 2
                    ? atoi(args[1])
                    : 0;

            exit(code);
        }

        if (is_box_command(
            args[0]
        )) {
            run_box(
                count,
                args
            );

            continue;
        }

        if (args[0][0] == '/') {
            run(
                args[0],
                count,
                args
            );

            continue;
        }

        char path[256];

        const char *prefix =
            "/bin/";

        size_t a =
            strlen(prefix);

        size_t b =
            strlen(args[0]);

        if (
            a + b + 1
            > sizeof(path)
        ) {
            printf(
                "kalush: command too long\n"
            );

            continue;
        }

        strcpy(
            path,
            prefix
        );

        strcpy(
            path + a,
            args[0]
        );

        if (test_file(path)) {
            char *program_args[
                MAX_ARGS
            ];

            program_args[0] =
                path;

            for (
                int i = 1;
                i < count;
                i++
            ) {
                program_args[i] =
                    args[i];
            }

            run(
                path,
                count,
                program_args
            );

            continue;
        }

        printf(
            "kalush: command not found: %s\n",
            args[0]
        );
    }
}
