# The Matrix Mode (--neo)

## Overview

The `--neo` flag transforms base-d into a Matrix-style terminal display, streaming random data encoded with the base256_matrix dictionary to create the iconic "falling code" effect.

## Usage

```bash
base-d --neo
```

Press `Ctrl+C` to exit.

## What It Does

1. **Displays iconic messages** - Types "Wake up, Neo..." and other messages character by character
2. **Loads base256_matrix dictionary** - The Matrix-style Japanese and geometric character set
3. **Generates random data** - Cross-platform random bytes using `rand` crate
4. **Encodes in real-time** - Converts random bytes to Matrix characters
5. **Streams to terminal** - Displays new line every 500ms
6. **Fills the screen** - Previous lines scroll up naturally

## Technical Details

### Random Data Source
- **Cross-platform**: Uses `rand::thread_rng()` instead of /dev/random
- **Works on**: Linux, macOS, Windows, BSD, etc.
- **Cryptographically secure**: Uses OS random source

### Display Characteristics
- **Update frequency**: 500ms (2 lines per second)
- **Terminal-aware**: Detects terminal width automatically
- **Fallback**: Uses 80 columns if detection fails
- **Encoding**: base256_matrix (1:1 byte-to-character)

### Performance
- **Efficient**: Only encodes one line at a time
- **Lightweight**: Minimal CPU usage
- **Smooth**: Regular 500ms intervals for visual effect

## Code Implementation

```rust
fn matrix_mode(config: &DictionariesConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Display iconic messages - typed character by character
    let messages = ["Wake up, Neo...", "The Matrix has you...", 
                    "Follow the white rabbit.", "Knock, knock, Neo."];
    
    for message in &messages {
        for ch in message.chars() {
            print!("{}", ch);
            stdout().flush()?;
            thread::sleep(Duration::from_millis(100));
        }
        thread::sleep(Duration::from_millis(800));
        print!("\r\x1b[K"); // Clear line
        stdout().flush()?;
    }
    
    // Stream encoded data
    loop {
        let mut random_bytes = vec![0u8; term_width / 2];
        rng.fill_bytes(&mut random_bytes);
        let encoded = encode(&random_bytes, &dictionary);
        println!("{}", encoded);
        thread::sleep(Duration::from_millis(500));
    }
}
```

## Visual Effect

The mode begins with iconic messages typed character by character:

```
Wake up, Neo...
(disappears)

The Matrix has you...
(disappears)

Follow the white rabbit.
(disappears)

Knock, knock, Neo.
(disappears)

(then streams encoded data)
ゎギチタザヅわプペホボポマミムメモャヤュユョヨラリルレロヮワヰヱヲンヴヵヶヷヸヹヺ・ーヽヾヿ
プペホボポマミムメモャヤュユョヨラリルレロヮワヰヱヲンヴヵヶヷヸヹヺ・ーヽヾヿプペホボポマミム
ホボポマミムメモャヤュユョヨラリルレロヮワヰヱヲンヴヵヶヷヸヹヺ・ーヽヾヿプペホボポマミムメモ
ョヨラリルレロヮワヰヱヲンヴヵヶヷヸヹヺ・ーヽヾヿプペホボポマミムメモャヤュユョヨラリルレロヮワ
...
```

## Features

### Iconic Intro Sequence
Messages appear character by character (100ms per character):
- "Wake up, Neo..."
- "The Matrix has you..."
- "Follow the white rabbit."
- "Knock, knock, Neo."

Each message displays, pauses, then clears before the next appears.

### Green Terminal Effect
The code uses ANSI escape sequences to set green text:
```rust
println!("\x1b[32m"); // Green text
```

### Screen Management
```rust
println!("\x1b[2J\x1b[H"); // Clear screen and move to top
print!("\r\x1b[K");         // Clear current line
```

### Graceful Exit
- Press `Ctrl+C` to exit
- Terminal returns to normal state
- No cleanup needed

## Dependencies

Added to `Cargo.toml`:
```toml
rand = "0.8"           # Cross-platform random number generation
terminal_size = "0.3"  # Terminal dimension detection
```

## Comparison with Other Matrix Tools

| Feature | cmatrix | neo | base-d --neo |
|---------|---------|-----|--------------|
| Platform | Unix-like | Node.js | Cross-platform Rust |
| Installation | apt/brew | npm | cargo |
| Encoding | N/A | N/A | Real base256 encoding |
| Random source | /dev/random | crypto | rand crate |
| Speed | Fast | Medium | Fast |
| Characters | Latin/Katakana | Various | base256_matrix dictionary |

## Use Cases

### 1. Terminal Screensaver
Use as a cool screensaver effect on your terminal.

### 2. Live Demonstrations
Show off base256_matrix encoding in action.

### 3. Testing
Visual verification that base256_matrix dictionary displays correctly.

### 4. Entertainment
Because regular terminals are boring.

### 5. Inspiration
Get in the mood for coding by entering The Matrix.

## Tips

### Full Screen Effect
```bash
# Clear screen first
clear && base-d --neo
```

### Capture Output
```bash
# Save 10 seconds of Matrix output
timeout 10 base-d --neo > matrix_output.txt
```

### Customize Speed
Currently fixed at 500ms. Future enhancement could add `--speed` flag:
```bash
# Proposed feature
base-d --neo --speed fast   # 100ms
base-d --neo --speed slow   # 1000ms
```

## Troubleshooting

### Issue: Characters Don't Display
**Solution**: Ensure your terminal font includes Japanese characters (Hiragana/Katakana)

### Issue: Terminal Width Wrong
**Solution**: Resize terminal and restart, or use default 80 columns

### Issue: Not Green
**Solution**: Some terminals don't support ANSI colors. Try a modern terminal emulator.

### Issue: Too Fast/Slow
**Solution**: Currently fixed at 500ms. Can be modified in source code.

## Easter Eggs

The feature is called `--neo` as a reference to the protagonist of The Matrix, who eventually learns to "see" the code. With base-d, you can now encode any data as Matrix-style falling code!

## Future Enhancements

1. **Color options**: `--neo-color green|red|blue`
2. **Speed control**: `--neo-speed <ms>`
3. **Column density**: `--neo-density <percent>`
4. **Character sets**: `--neo-style matrix|katakana|hiragana|mixed`
5. **Data source**: `--neo-input <file>` to encode specific data
6. **Animation**: Simulate actual "falling" effect with coordinate tracking

## Philosophy

*"You take the blue pill, the story ends. You wake up in your bed and believe whatever you want to believe. You take the red pill, you stay in Wonderland, and I show you how deep the rabbit hole goes... or you use `--neo` and encode random data as Matrix-style falling code."*

Welcome to the real world, Neo.
