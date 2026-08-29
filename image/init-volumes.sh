#!/bin/bash
# Docker creates volume mountpoints as root. Hand the per-worktree volumes that
# sit inside the workspace (node_modules etc.) to the `node` user. Only touches
# mounts that paddock declared via the PADDOCK_VOLUME_TARGETS file.
set -euo pipefail
LIST=/commandhistory/.paddock-volume-targets
[ -f "$LIST" ] || exit 0
while read -r target; do
    [ -z "$target" ] && continue
    if mountpoint -q "$target"; then
        chown node:node "$target"
        echo "chown node:node $target"
    fi
done < "$LIST"
