#!/usr/bin/env bash

set -e
make -C userland clean
    make -C userland
IMAGE="disk.img"
SIZE_MB=64
USERLAND="userland/init.elf"

if [ ! -f "$USERLAND" ]; then
    echo "W: Userland not found running make..."
    exit 1
fi

rm -f "$IMAGE"

dd if=/dev/zero of="$IMAGE" bs=1M count="$SIZE_MB" status=progress

mkfs.ext2 -F "$IMAGE"

debugfs -w "$IMAGE" <<EOF
mkdir /bin
mkdir /etc
mkdir /home
mkdir /tmp
mkdir /dev
write $USERLAND /bin/init
EOF
debugfs -R "ls -l /bin" "$IMAGE"
