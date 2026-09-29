#include <stdio.h>
#include <unistd.h>

static int start_shell(void)
{
    char *argv[] = {
        "/bin/kalush"
    };

    return spawn("/bin/kalush", 1, argv, 5);
}

int main(int argc, char **argv)
{
    (void)argc;
    (void)argv;

    puts("KaluOS init starting");

    for (;;) {
        int pid = start_shell();

        if (pid < 0) {
            puts("init: failed to start /bin/kalush");
            sleep(1);
            continue;
        }

        printf("init: kalush started pid=%d\n", pid);

        int status = 0;

        for (;;) {
            int result = wait(pid, &status);

            if (result == pid)
                break;

            yield();
        }

        printf(
            "init: kalush exited status=%d\n",
            status
        );

        sleep(1);
    }

    return 0;
}
