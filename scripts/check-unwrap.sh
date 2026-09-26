#!/usr/bin/env bash
# Gate: no unwrap/expect/panic in non-test Rust.
#
# Production scope only (as decided): blocks that start at a `#[cfg(test)]`
# attribute are stripped; integration-test directories are skipped entirely.
# Clippy (deny unwrap_used/expect_used) enforces the same rule at compile time.
set -euo pipefail
cd "$(dirname "$0")/.."

scan_dirs=(services/user-service/src services/messenger-service/src crates/db-core/src crates/events/src)

violations=$(find "${scan_dirs[@]}" -name '*.rs' -type f | sort | while IFS= read -r file; do
  awk -v file="$file" '
    # A `#[cfg(test)]` attribute starts a unit-test block. Skip lines until the
    # braces balance: counting starts at the first `{` (e.g. `mod tests {`),
    # and the block ends when the count returns to zero.
    /#\[cfg\(test\)\]/ { in_test = 1; depth = 0; saw_open = 0; next }
    in_test {
        opens = gsub(/\{/, "{")
        closes = gsub(/\}/, "}")
        if (opens > 0) saw_open = 1
        depth += opens - closes
        if (saw_open && depth <= 0) { in_test = 0 }
        next
    }
    /unwrap\(|expect\(|panic!|unreachable!|todo!|unimplemented!/ {
        print file ":" FNR ": " $0
    }
  ' "$file"
done)

if [ -n "$violations" ]; then
    echo "unwrap/expect/panic found in non-test code:" >&2
    echo "$violations" >&2
    exit 1
fi
echo "no unwrap/expect/panic in production code"
