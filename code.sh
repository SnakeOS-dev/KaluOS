#!/bin/bash
for file in $(find "${1:-kernel/src}" -type f \( -name "*.rs" -o -name "*.h" -o -name "*.asm" -o -name "*.md" -o -name "*.txt" -o -name "*.sh" -o -name "Makefil*" -o -name "*.S" -o -name "*.s" \)); do
    echo "===== start of $file =====" && cat "$file" && echo "===== end of $file ====="
done
