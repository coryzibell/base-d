use std::fs::File;
use std::io::Write;

fn main() -> std::io::Result<()> {
    let mut chars = Vec::new();

    // Strategy: Create a Matrix-style 256-character dictionary
    // Using Katakana, Hiragana, and Unicode shapes that look "Matrix-like"

    // 1. Hiragana (83 chars) - U+3041 to U+3093
    // ぁあぃいぅうぇえぉおかがきぎくぐけげこごさざしじすずせぜそぞただちぢっつづてでとどなにぬねのはばぱひびぴふぶぷへべぺほぼぽまみむめもゃやゅゆょよらりるれろゎわゐゑをんゔゕゖ゗゘゙゚゛゜ゝゞゟ
    for i in 0x3041..=0x3093 {
        if let Some(c) = char::from_u32(i) {
            chars.push(c);
        }
    }

    // 2. Katakana (96 chars) - U+30A0 to U+30FF
    // ゠ァアィイゥウェエォオカガキギクグケゲコゴサザシジスズセゼソゾタダチヂッツヅテデトドナニヌネノハバパヒビピフブプヘベペホボポマミムメモャヤュユョヨラリルレロヮワヰヱヲンヴヵヶヷヸヹヺ・ーヽヾヿ
    for i in 0x30A0..=0x30FF {
        if let Some(c) = char::from_u32(i) {
            chars.push(c);
        }
    }

    // 3. Box Drawing (32 chars) - U+2500 to U+251F
    // ─━│┃┄┅┆┇┈┉┊┋┌┍┎┏┐┑┒┓└┕┖┗┘┙┚┛├┝┞┟
    for i in 0x2500..=0x251F {
        if let Some(c) = char::from_u32(i) {
            chars.push(c);
        }
    }

    // 4. Geometric Shapes (16 chars) - U+25A0 to U+25AF
    // ■□▢▣▤▥▦▧▨▩▪▫▬▭▮▯
    for i in 0x25A0..=0x25AF {
        if let Some(c) = char::from_u32(i) {
            chars.push(c);
        }
    }

    // 5. Block Elements (32 chars) - U+2580 to U+259F
    // ▀▁▂▃▄▅▆▇█▉▊▋▌▍▎▏▐░▒▓▔▕▖▗▘▙▚▛▜▝▞▟
    for i in 0x2580..=0x259F {
        if let Some(c) = char::from_u32(i) {
            chars.push(c);
        }
    }

    // Calculate how many more we need to reach 256
    let remaining = if chars.len() < 256 {
        256 - chars.len()
    } else {
        0
    };
    println!("Have {} characters, need {} more", chars.len(), remaining);

    // 6. Fill remaining with Mathematical Operators and symbols
    if remaining > 0 {
        // Mathematical Operators
        for i in 0x2200u32..0x2200u32 + remaining.min(32) as u32 {
            if chars.len() >= 256 {
                break;
            }
            if let Some(c) = char::from_u32(i) {
                if !c.is_control() && !c.is_whitespace() {
                    chars.push(c);
                }
            }
        }
    }

    // Trim to exactly 256 characters
    chars.truncate(256);

    println!(
        "Generated {} characters for Matrix-style base256 dictionary",
        chars.len()
    );

    // Create the dictionary string
    let alphabet_str: String = chars.iter().collect();

    // Write to file
    let mut file = File::create("base256_matrix.txt")?;
    writeln!(file, "[alphabets.base256_matrix]")?;
    writeln!(file, "chars = \"{}\"", alphabet_str)?;
    writeln!(
        file,
        "mode = \"chunked\"  # Can also use 'base_conversion' - both produce identical output!"
    )?;
    writeln!(file, "# Matrix-style 256-character dictionary")?;
    writeln!(
        file,
        "# Uses: Hiragana, Katakana, Box Drawing, Geometric Shapes, Block Elements"
    )?;
    writeln!(file, "# Special property: 8 bits % log2(256) = 8 % 8 = 0")?;
    writeln!(
        file,
        "# This means chunked and mathematical modes produce IDENTICAL output!"
    )?;
    writeln!(
        file,
        "# Like hexadecimal, but with Matrix-style characters!"
    )?;

    println!("Written to base256_matrix.txt");
    println!("Character count: {}", chars.len());
    println!();
    println!("🟢 Matrix Base256 is special like hexadecimal:");
    println!("  - 1 character = 8 bits = 1 byte (log2(256) = 8)");
    println!("  - 8 bits % 8 = 0 (perfect division)");
    println!("  - Both encoding modes produce IDENTICAL output!");
    println!("  - Uses Matrix-style Japanese and geometric characters");
    println!();
    println!("Character breakdown:");
    println!("  - Hiragana: ~83 characters");
    println!("  - Katakana: ~96 characters");
    println!("  - Box Drawing: ~32 characters");
    println!("  - Geometric Shapes: ~16 characters");
    println!("  - Block Elements: ~32 characters");
    println!("  - Other symbols: ~remaining");

    Ok(())
}
