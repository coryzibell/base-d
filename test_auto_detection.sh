#!/usr/bin/env bash
# Test auto-detection for fiche encoding
# Expected: Full mode for tabular, Path mode for complex nested/varying structures

set -e

BD="cargo run --release --quiet -- fiche encode"

echo "=== Test 1: Homogeneous array (expect Full - tabular) ==="
echo '[{"name":"alice"},{"name":"bob"}]' | $BD
echo

echo "=== Test 2: Varying array structure (expect Path) ==="
echo '{"items":[{"type":"a","x":1},{"type":"b","y":2}]}' | $BD
echo

echo "=== Test 3: Deep nested with arrays (expect Path) ==="
echo '{"a":{"b":{"c":[{"d":1},{"d":2}]}}}' | $BD
echo

echo "=== Test 4: Tabular with wrapper key (expect Full) ==="
echo '{"results":[{"id":1,"value":"x"},{"id":2,"value":"y"}]}' | $BD
echo

echo "=== Test 5: Nested objects depth=3 (expect Path) ==="
echo '{"users":[{"id":1,"profile":{"name":"alice","age":30}},{"id":2,"profile":{"name":"bob","age":25}}]}' | $BD
echo

echo "=== Test 6: Simple object (expect Full) ==="
echo '{"id":1,"name":"alice"}' | $BD
echo

echo "All tests completed."
