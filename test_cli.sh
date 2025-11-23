#!/bin/bash
# Test script for base-d CLI

set -e

BIN="./target/release/base-d"

echo "=== base-d CLI Tests ==="
echo

# Test 1: Help
echo "✓ Testing --help"
$BIN --help > /dev/null

# Test 2: List alphabets
echo "✓ Testing --list"
$BIN --list | grep -q "cards"

# Test 3: Encode from stdin
echo "✓ Testing encode from stdin"
result=$(echo "Test" | $BIN)
[ -n "$result" ]

# Test 4: Round-trip with default alphabet
echo "✓ Testing round-trip (cards)"
original="Hello, World!"
encoded=$(echo "$original" | $BIN)
decoded=$(echo "$encoded" | $BIN -d)
[ "$decoded" = "$original" ]

# Test 5: DNA alphabet
echo "✓ Testing DNA alphabet"
result=$(echo "ACGT" | $BIN -a dna)
decoded=$(echo "$result" | $BIN -a dna -d)
[ "$decoded" = "ACGT" ]

# Test 6: Binary alphabet
echo "✓ Testing binary alphabet"
result=$(echo "A" | $BIN -a binary)
decoded=$(echo "$result" | $BIN -a binary -d)
[ "$decoded" = "A" ]

# Test 7: File input
echo "✓ Testing file input"
echo "File test" > /tmp/test_base_d.txt
result=$($BIN /tmp/test_base_d.txt)
[ -n "$result" ]
decoded=$(echo "$result" | $BIN -d)
[ "$decoded" = "File test" ]
rm /tmp/test_base_d.txt

# Test 8: Binary data
echo "✓ Testing binary data preservation"
printf "\x00\x01\xff" > /tmp/test_binary.bin
encoded=$($BIN /tmp/test_binary.bin)
echo "$encoded" | $BIN -d > /tmp/test_decoded.bin
cmp -s /tmp/test_binary.bin /tmp/test_decoded.bin
rm /tmp/test_binary.bin /tmp/test_decoded.bin

echo
echo "All tests passed! ✓"
