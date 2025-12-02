//! CLI integration tests for base-d
//!
//! Tests the binary as a user would interact with it.

use assert_cmd::Command;
use predicates::prelude::*;

#[allow(deprecated)]
fn base_d() -> Command {
    Command::cargo_bin("base-d").expect("binary exists")
}

// ============================================================================
// Basic Commands
// ============================================================================

#[test]
fn test_help() {
    base_d()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Universal multi-dictionary encoder",
        ));
}

#[test]
fn test_version() {
    base_d()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("base-d"));
}

#[test]
fn test_config_dictionaries() {
    base_d()
        .args(["config", "list", "dictionaries"])
        .assert()
        .success()
        .stdout(predicate::str::contains("base64"));
}

#[test]
fn test_config_compression() {
    base_d()
        .args(["config", "list", "algorithms"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gzip"))
        .stdout(predicate::str::contains("zstd"));
}

#[test]
fn test_config_hash() {
    base_d()
        .args(["config", "list", "hashes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sha256"))
        .stdout(predicate::str::contains("blake3"));
}

// ============================================================================
// Encode/Decode Round-trips
// ============================================================================

#[test]
fn test_encode_base64() {
    base_d()
        .args(["encode", "base64"])
        .write_stdin("hello world")
        .assert()
        .success()
        .stdout("aGVsbG8gd29ybGQ=\n");
}

#[test]
fn test_decode_base64() {
    base_d()
        .args(["decode", "base64"])
        .write_stdin("aGVsbG8gd29ybGQ=")
        .assert()
        .success()
        .stdout("hello world");
}

#[test]
fn test_roundtrip_base64() {
    // Encode
    let encoded = base_d()
        .args(["encode", "base64"])
        .write_stdin("test data 123")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Decode
    base_d()
        .args(["decode", "base64"])
        .write_stdin(encoded)
        .assert()
        .success()
        .stdout("test data 123");
}

#[test]
fn test_roundtrip_base32() {
    let encoded = base_d()
        .args(["encode", "base32"])
        .write_stdin("hello")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    base_d()
        .args(["decode", "base32"])
        .write_stdin(encoded)
        .assert()
        .success()
        .stdout("hello");
}

#[test]
fn test_roundtrip_hex() {
    base_d()
        .args(["encode", "base16"])
        .write_stdin("ABC")
        .assert()
        .success()
        .stdout("414243\n");

    base_d()
        .args(["decode", "base16"])
        .write_stdin("414243")
        .assert()
        .success()
        .stdout("ABC");
}

// ============================================================================
// Compression
// ============================================================================

#[test]
fn test_compress_gzip_roundtrip() {
    let compressed = base_d()
        .args(["encode", "base64", "--compress", "gzip"])
        .write_stdin("compress me please")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    base_d()
        .args(["decode", "base64", "--decompress", "gzip"])
        .write_stdin(compressed)
        .assert()
        .success()
        .stdout("compress me please");
}

#[test]
fn test_compress_zstd_roundtrip() {
    let compressed = base_d()
        .args(["encode", "base64", "--compress", "zstd"])
        .write_stdin("zstd compression test")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    base_d()
        .args(["decode", "base64", "--decompress", "zstd"])
        .write_stdin(compressed)
        .assert()
        .success()
        .stdout("zstd compression test");
}

// ============================================================================
// Hashing
// ============================================================================

#[test]
fn test_hash_sha256() {
    // Hash output is encoded with dejavu (random dictionary)
    base_d()
        .args(["hash", "sha256"])
        .write_stdin("hello")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_hash_md5() {
    // Hash output is encoded with dejavu (random dictionary)
    base_d()
        .args(["hash", "md5"])
        .write_stdin("hello")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_hash_blake3() {
    base_d()
        .args(["hash", "blake3"])
        .write_stdin("hello")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// ============================================================================
// Error Handling
// ============================================================================

#[test]
fn test_invalid_dictionary() {
    base_d()
        .args(["encode", "nonexistent_dict"])
        .write_stdin("test")
        .assert()
        .failure();
}

#[test]
fn test_file_not_found() {
    base_d()
        .args(["encode", "base64", "/nonexistent/path/file.txt"])
        .assert()
        .failure();
}

#[test]
fn test_invalid_base64_decode() {
    base_d()
        .args(["decode", "base64"])
        .write_stdin("not valid base64!!!")
        .assert()
        .failure();
}

// ============================================================================
// NO_COLOR Support
// ============================================================================

#[test]
fn test_no_color_flag() {
    // Should succeed without error when --no-color is passed
    base_d().args(["--no-color", "--help"]).assert().success();
}

#[test]
fn test_no_color_env() {
    // Should succeed with NO_COLOR env var set
    base_d()
        .env("NO_COLOR", "1")
        .args(["--help"])
        .assert()
        .success();
}

// ============================================================================
// Size Limits
// ============================================================================

#[test]
fn test_max_size_flag() {
    // Small input should work with default limit
    base_d()
        .args(["encode", "base64"])
        .write_stdin("small input")
        .assert()
        .success();
}

#[test]
fn test_max_size_zero_unlimited() {
    // --max-size 0 should allow unlimited
    base_d()
        .args(["--max-size", "0", "encode", "base64"])
        .write_stdin("test")
        .assert()
        .success();
}

// ============================================================================
// Detection
// ============================================================================

#[test]
fn test_detect_base64() {
    // detect decodes the input and outputs the result
    base_d()
        .args(["detect"])
        .write_stdin("aGVsbG8gd29ybGQ=")
        .assert()
        .success()
        .stdout("hello world");
}

#[test]
fn test_detect_with_candidates() {
    base_d()
        .args(["detect", "--show-candidates", "3"])
        .write_stdin("aGVsbG8=")
        .assert()
        .success();
}

// ============================================================================
// Schema Encoding/Decoding
// ============================================================================

#[test]
fn test_schema_encode_stdin() {
    base_d()
        .arg("schema")
        .write_stdin(r#"{"id":1,"name":"alice"}"#)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("𓍹"))
        .stdout(predicate::str::ends_with("𓍺\n"));
}

#[test]
fn test_schema_decode_stdin() {
    // First encode to get valid schema output
    let encode_result = base_d()
        .arg("schema")
        .write_stdin(r#"{"id":1,"name":"alice"}"#)
        .output()
        .expect("encode succeeded");

    let encoded = String::from_utf8(encode_result.stdout).expect("valid utf8");

    // Now decode it back
    base_d()
        .args(["schema", "-d"])
        .write_stdin(encoded.trim())
        .assert()
        .success()
        .stdout(predicate::str::contains("id"))
        .stdout(predicate::str::contains("alice"));
}

#[test]
fn test_schema_roundtrip() {
    let input = r#"{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}"#;

    // Encode
    let encode_result = base_d()
        .arg("schema")
        .write_stdin(input)
        .output()
        .expect("encode succeeded");

    let encoded = String::from_utf8(encode_result.stdout).expect("valid utf8");

    // Decode
    let decode_result = base_d()
        .args(["schema", "-d"])
        .write_stdin(encoded.trim())
        .output()
        .expect("decode succeeded");

    let decoded = String::from_utf8(decode_result.stdout).expect("valid utf8");

    // Compare as JSON values (order-independent)
    let input_value: serde_json::Value = serde_json::from_str(input).expect("valid input json");
    let output_value: serde_json::Value =
        serde_json::from_str(decoded.trim()).expect("valid output json");

    assert_eq!(input_value, output_value);
}

#[test]
fn test_schema_help() {
    base_d()
        .args(["schema", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Schema encoding"))
        .stdout(predicate::str::contains("--decode"));
}
