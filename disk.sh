#!/usr/bin/env bash

set -e

IMAGE="disk.img"
make -C userland clean
make -C userland

rm -f "$IMAGE"

dd \
    if=/dev/zero \
    of="$IMAGE" \
    bs=1M \
    count=64 \
    status=progress

mkfs.ext2 \
    -F \
    "$IMAGE"

debugfs -w "$IMAGE" <<EOF
mkdir /bin
mkdir /etc
mkdir /home
mkdir /tmp
mkdir /dev
write userland/init.elf /bin/init
write userland/kalush.elf /bin/kalush
write userland/kalubox.elf /bin/kalubox
EOF

debugfs \
    -R "ls -l /bin" \
    "$IMAGE"
