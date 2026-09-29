#!/bin/bash
for file in $(find kernel/src userland -type f \( -name "*.rs" -o -name "*.h" -o -name "*.c"-o -name "*.asm" -o -name "*.md" -o -name "*.txt" -o -name "*.sh" -o -name "Makefil*" -o -name "*.S" -o -name "*.s" \)); do
    echo "===== start of $file =====" && cat "$file" && echo "===== end of $file ====="
done
