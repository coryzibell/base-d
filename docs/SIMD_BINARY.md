# SIMD for Binary Encoding

## Current Status

No SIMD implementation for binary (base-2) encoding.

## Current SIMD Support

| Base | Bits/char | Implementation |
|------|-----------|----------------|
| base16 | 4-bit | SmallLutCodec |
| base32 | 5-bit | Specialized + GappedLutCodec |
| base64 | 6-bit | Specialized + GappedLutCodec |
| base256 | 8-bit | Specialized |

## Why Binary SIMD Is Problematic

### The Expansion Problem

Binary encoding has the worst expansion ratio of any base:
- 1 input byte → 8 output characters
- 16 bytes (one SIMD register) → 128 output bytes
- 32 bytes (AVX2 register) → 256 output bytes

Compare to base64:
- 3 input bytes → 4 output characters (1.33x expansion)

### Memory Bandwidth Dominates

SIMD shines when computation is the bottleneck. With binary:
1. **Read**: 32 bytes
2. **Compute**: Extract bits (trivial)
3. **Write**: 256 bytes

The 8x write amplification means you're memory-bound, not compute-bound.

### Bit Extraction Is Already Fast

Scalar binary encoding is simple:
```rust
for byte in data {
    for i in (0..8).rev() {
        result.push(if (byte >> i) & 1 == 1 { '1' } else { '0' });
    }
}
```

There's no complex LUT lookup or range validation to accelerate.

### Potential SIMD Approach

If implemented, it would use:
- `movmskb` / `pmovmskb` to extract bit masks
- Parallel bit-to-ASCII conversion (`0x30` + bit value)
- Interleaving for correct output order

But the gains would be marginal due to the expansion ratio.

## Conclusion

SIMD for binary encoding is technically possible but economically poor:
- The 8x expansion ratio makes it memory-bound
- Scalar bit shifts are already fast
- Code complexity isn't justified by performance gains

The current scalar implementation is likely within 2x of theoretical SIMD performance, whereas base64 SIMD provides 4-10x speedup over scalar.

## Future Consideration

If profiling shows binary encoding as a bottleneck in real workloads, revisit this analysis. Until then, scalar is sufficient.
