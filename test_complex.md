# Complex Markdown Test

This document tests the markdown table parser against **real-world markdown** with all the bells and whistles.

## Introduction

Here's some introductory text with *italics*, **bold**, and ***bold italics***. We also have `inline code` and ~~strikethrough~~.

### Bullet Lists

- Item one
- Item two
  - Nested item
  - Another nested
- Item three

### Numbered Lists

1. First thing
2. Second thing
   1. Sub-item A
   2. Sub-item B
3. Third thing

## The Data Table

Here's the actual table we want to parse:

| id | name    | score | active | rate  | notes           |
|:---|:-------:|------:|--------|-------|-----------------|
| A1 | Alice   | 95    | true   | 3.14  | Top performer   |
| B2 | Bob     | -12   | false  | 2.718 | Needs improvement |
| C3 | Charlie | 100   | TRUE   | 0.5   |                 |
| D4 |         | 42    | False  | -1.5  | Missing name    |

### Analysis

The table above contains:
- **Strings**: id, name, notes
- **Integers**: score (mixed positive/negative → I64)
- **Booleans**: active (case variations)
- **Floats**: rate (including negative)
- **Nulls**: empty cells

## Code Block

```rust
fn main() {
    println!("This should be ignored");
}
```

## Another Table (Should Be Ignored If We Only Parse First)

| ignore | this |
|--------|------|
| x      | y    |

## Blockquote

> This is a blockquote.
> It spans multiple lines.
>
> > Nested blockquote

## Links and Images

[Link text](https://example.com)
![Alt text](image.png)

---

## Conclusion

That's all folks! The parser should extract the **first valid table** and ignore everything else.
