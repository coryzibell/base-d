# Dictionary Auto-Detection

base-d can automatically detect which dictionary was used to encode data, eliminating the need to manually specify the encoding format.

## Overview

The detection system analyzes input text using multiple heuristics to identify the most likely dictionary:
- Character set matching
- Dictionary specificity (prefer common standards)
- Padding detection (for RFC formats)
- Length validation
- Decode validation (actually try decoding)

## CLI Usage

### Basic Detection

```bash
# Auto-detect and decode
echo "SGVsbG8sIFdvcmxkIQ==" | base-d --detect
# Output: Hello, World!
# Stderr: Detected: base64 (confidence: 75.4%)
```

### Show Candidates

Display top N matching dictionaries with confidence scores:

```bash
base-d --detect --show-candidates 5 encoded.txt
# Output:
# Top 5 candidate dictionaries:
#
# 1. base64 (confidence: 75.4%)
# 2. base64url (confidence: 75.4%)
# 3. z85 (confidence: 75.0%)
# 4. base85 (confidence: 73.6%)
# 5. ascii85 (confidence: 71.2%)
```

### With Decompression

Combine detection with decompression:

```bash
# Data was compressed with gzip, then encoded (but we don't know which dictionary)
cat mystery.txt | base-d --detect --decompress gzip
```

### From Files

```bash
# Detect from file
base-d --detect encoded_file.txt

# Detect and save output
base-d --detect input.txt > decoded_output.bin
```

## Library API

### Basic Detection

```rust
use base_d::detect_dictionary;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = "SGVsbG8sIFdvcmxkIQ==";
    
    // Get ranked list of matches
    let matches = detect_dictionary(input)?;
    
    if let Some(best_match) = matches.first() {
        println!("Detected: {} ({:.1}% confidence)", 
            best_match.name,
            best_match.confidence * 100.0
        );
        
        // Use the detected dictionary to decode
        let decoded = base_d::decode(input, &best_match.dictionary)?;
        println!("Decoded: {}", String::from_utf8_lossy(&decoded));
    }
    
    Ok(())
}
```

### Advanced Usage with DictionaryDetector

```rust
use base_d::{DictionariesConfig, DictionaryDetector, decode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = DictionariesConfig::load_with_overrides()?;
    
    // Create detector (reuse for multiple detections)
    let detector = DictionaryDetector::new(&config)?;
    
    // Detect multiple inputs
    let inputs = vec![
        "SGVsbG8=",           // base64
        "48656c6c6f",         // hex
        "HelloWorld",         // base85
    ];
    
    for input in inputs {
        let matches = detector.detect(input);
        
        if let Some(best) = matches.first() {
            println!("{} -> {} ({:.0}%)", 
                input,
                best.name,
                best.confidence * 100.0
            );
        }
    }
    
    Ok(())
}
```

## Detection Accuracy

### High Confidence Cases (>80%)

**RFC Standards with Padding:**
- base64: `SGVsbG8sIFdvcmxkIQ==`
- base32: `JBSWY3DPEBLW64TMMQ======`
- hex: `48656c6c6f`

**Distinctive Character Sets:**
- Binary: `01001000` (only 0 and 1)
- DNA: `ACGTACGT` (only A, C, G, T)

### Medium Confidence Cases (60-80%)

**Similar Dictionaries:**
- base64 vs base64url (differ by 2 characters)
- base32 vs base32hex (different character sets)
- hex vs hex_math (same characters, different mode)

**Short Inputs:**
- Less than 10 characters
- Not enough statistical information

### Low Confidence Cases (<60%)

**Overlapping Character Sets:**
- base85 and z85 both contain all base64 characters
- base62 contains all hex characters

**Ambiguous Data:**
- All-uppercase alphanumeric (could be base32, base58, etc.)
- Very short strings (1-5 characters)

## How Detection Works

### 1. Character Set Matching (25% weight)

Checks if all input characters exist in the dictionary. Also considers dictionary usage ratio - if only 20% of a dictionary's characters are used, it's probably the wrong dictionary.

### 2. Specificity Scoring (20% weight)

Prefers smaller, more common dictionaries:
- hex (16 chars): 1.0
- base32 (32 chars): 0.95
- base64 (64 chars): 0.92
- base85 (85 chars): 0.70

### 3. Padding Detection (30% weight)

For chunked modes (RFC standards):
- Checks for `=` padding at end
- Validates padding isn't in the middle
- Confirms reasonable padding count (<= 3)

### 4. Length Validation (15% weight)

Chunked modes have specific length requirements:
- base64: multiple of 4
- base32: multiple of 8
- base16: multiple of 2

### 5. Decode Validation (10% weight)

Attempts to actually decode the input. If decoding fails, confidence drops to 0.

## Limitations

### Cannot Detect

1. **Custom user dictionaries** - Only built-in dictionaries are checked
2. **Nested encoding** - e.g., base64(base64(data))
3. **Mixed encodings** - Different parts encoded differently
4. **Corrupted data** - May detect wrong dictionary for invalid input

### Ambiguity

Some dictionary pairs are nearly identical:
- `base64` vs `base64url` (only differ in 2 characters: `+/` vs `-_`)
- `base32` vs `base32hex` (different character sets but similar properties)

In these cases, detection returns multiple high-confidence matches.

### Short Inputs

Detection accuracy decreases with input length:
- **1-5 chars**: Very unreliable
- **6-15 chars**: Moderate reliability
- **16+ chars**: High reliability
- **50+ chars**: Excellent reliability

## Tips for Best Results

### Provide More Data

```bash
# Good: Long input
base-d --detect long_encoded_file.txt

# Poor: Very short input  
echo "ABC" | base-d --detect
```

### Use --show-candidates for Ambiguous Cases

```bash
# See all likely matches
base-d --detect --show-candidates 10 input.txt
```

### Validate Results

If confidence is low (<70%), manually verify:

```bash
base-d --detect input.txt 2>&1 | grep "confidence"
# Output: Detected: base64 (confidence: 45.2%)
# Warning: Low confidence detection. Results may be incorrect.
```

### Known Dictionary? Specify It

Detection is a convenience feature. If you know the dictionary, specify it:

```bash
# Faster and guaranteed correct
base-d -d base64 input.txt

# vs slower with potential error
base-d --detect input.txt
```

## Performance

Detection is fast:
- **Typical**: <1ms for 100 characters
- **Large**: ~5ms for 10KB input
- **Caching**: Create `DictionaryDetector` once, reuse many times

## Examples

### Example 1: Unknown Format

```bash
$ cat mystery.txt
VGhpcyBpcyBhIHRlc3Q=

$ base-d --detect mystery.txt
Detected: base64 (confidence: 75.4%)
This is a test
```

### Example 2: Multiple Candidates

```bash
$ echo "ABCDEF123456" | base-d --detect --show-candidates 5
Top 5 candidate dictionaries:

1. hex_math (confidence: 89.3%)
2. hex (confidence: 89.3%)
3. base32_zbase (confidence: 80.7%)
4. base62 (confidence: 79.9%)
5. base64_math (confidence: 78.3%)
```

### Example 3: With Compression

```bash
$ echo "Some data" | base-d --compress gzip -e base64 > compressed.txt

$ base-d --detect --decompress gzip compressed.txt
Detected: base64 (confidence: 75.4%)
Some data
```

### Example 4: Batch Processing

```bash
# Detect and decode all .enc files
for file in *.enc; do
    base-d --detect "$file" > "${file%.enc}.txt"
done
```

## Future Enhancements

Planned improvements (see ROADMAP.md):
- Machine learning model for better accuracy
- Detection of compression (automatically decompress)
- Detection of nested encodings
- Training on custom user dictionaries
- Confidence calibration based on input length
