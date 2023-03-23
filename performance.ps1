param(
    [Parameter()]
    [String]$SCAN_PATH
)

$SPACE_DISPLAY_CMD = "$pwd/target/release/spacedisplay.exe --no-ui $SCAN_PATH"
$DIR_CMD = "dir $SCAN_PATH /s"

cargo build --release

$COMPARE_FILE = "target/COMPARE.md"

Write-Output "## Windows" | Out-File $COMPARE_FILE -Encoding utf8

hyperfine --warmup 5 -m 10 -u second `
  --export-markdown "target/compare-temp.md" `
  -n spacedisplay `
  "$SPACE_DISPLAY_CMD" `
  -n "dir /s" `
  "$DIR_CMD"

Get-Content "target/compare-temp.md" -Encoding utf8 | Out-File $COMPARE_FILE -Encoding utf8 -Append
Remove-Item -Force "target/compare-temp.md"
