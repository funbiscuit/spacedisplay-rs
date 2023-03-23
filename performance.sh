#!/bin/bash

set -eu -o pipefail
SCAN_PATH=$1
SPACE_DISPLAY_CMD="target/release/spacedisplay --no-ui $SCAN_PATH"
# du will exit with non zero code if got permission denied on some paths
DU_CMD="du -sh $SCAN_PATH || exit 0"

cargo build --release

compare_file="target/COMPARE.md"

echo "## $(uname -sr)" >$compare_file

hyperfine --warmup 5 -m 10 \
  --export-markdown "target/compare-temp.md" \
  -n spacedisplay \
  "$SPACE_DISPLAY_CMD" \
  -n "du -sh" \
  "$DU_CMD"

cat "target/compare-temp.md" >>$compare_file
rm -f "target/compare-temp.md"
