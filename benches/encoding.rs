use base_d::{
    Dictionary, DictionaryRegistry, EncodingMode,
    bench::{
        EncodingPath, PlatformInfo, decode_with_path, detect_available_paths, encode_with_path,
    },
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

/// Test data sizes for benchmarking
const SIZES: &[usize] = &[64, 256, 1024, 4096, 16384, 65536];

fn get_dictionary(name: &str) -> Option<Dictionary> {
    let config = DictionaryRegistry::load_default().ok()?;
    let dictionary_config = config.get_dictionary(name)?;
    let effective_mode = dictionary_config.effective_mode();

    match effective_mode {
        EncodingMode::ByteRange => {
            let start = dictionary_config.start_codepoint?;
            Dictionary::builder()
                .chars(Vec::new())
                .mode(effective_mode)
                .start_codepoint(start)
                .build()
                .ok()
        }
        _ => {
            let chars: Vec<char> = dictionary_config.effective_chars().ok()?.chars().collect();
            let padding = dictionary_config
                .padding
                .as_ref()
                .and_then(|s| s.chars().next());
            let mut builder = Dictionary::builder().chars(chars).mode(effective_mode);
            if let Some(pad) = padding {
                builder = builder.padding(pad);
            }
            builder.build().ok()
        }
    }
}

fn generate_random_data(size: usize) -> Vec<u8> {
    // Use a simple PRNG for reproducible "random" data
    let mut data = Vec::with_capacity(size);
    let mut state: u64 = 0xDEADBEEF;
    for _ in 0..size {
        // Simple xorshift
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        data.push(state as u8);
    }
    data
}

/// Benchmark encoding for a single dictionary across all available paths
fn bench_encode_dictionary(c: &mut Criterion, dict_name: &str) {
    let Some(dictionary) = get_dictionary(dict_name) else {
        eprintln!("Skipping {}: dictionary not found", dict_name);
        return;
    };

    let paths = detect_available_paths(&dictionary);
    let mut group = c.benchmark_group(format!("encode/{}", dict_name));

    for &size in SIZES {
        let data = generate_random_data(size);
        group.throughput(Throughput::Bytes(size as u64));

        for &path in &paths {
            let id = BenchmarkId::new(path.to_string(), size);
            group.bench_with_input(id, &data, |b, data| {
                b.iter(|| encode_with_path(black_box(data), black_box(&dictionary), path));
            });
        }
    }

    group.finish();
}

/// Benchmark decoding for a single dictionary across all available paths
fn bench_decode_dictionary(c: &mut Criterion, dict_name: &str) {
    let Some(dictionary) = get_dictionary(dict_name) else {
        eprintln!("Skipping {}: dictionary not found", dict_name);
        return;
    };

    let paths = detect_available_paths(&dictionary);
    let mut group = c.benchmark_group(format!("decode/{}", dict_name));

    for &size in SIZES {
        let data = generate_random_data(size);
        // Pre-encode using scalar (guaranteed to work)
        let Some(encoded) = encode_with_path(&data, &dictionary, EncodingPath::Scalar) else {
            eprintln!("Skipping decode/{} size {}: encode failed", dict_name, size);
            continue;
        };

        group.throughput(Throughput::Bytes(size as u64));

        for &path in &paths {
            let id = BenchmarkId::new(path.to_string(), size);
            group.bench_with_input(id, &encoded, |b, encoded| {
                b.iter(|| decode_with_path(black_box(encoded), black_box(&dictionary), path));
            });
        }
    }

    group.finish();
}

// Individual benchmark functions for criterion_group
fn bench_base64(c: &mut Criterion) {
    bench_encode_dictionary(c, "base64");
    bench_decode_dictionary(c, "base64");
}

fn bench_base64url(c: &mut Criterion) {
    bench_encode_dictionary(c, "base64url");
    bench_decode_dictionary(c, "base64url");
}

fn bench_base32(c: &mut Criterion) {
    bench_encode_dictionary(c, "base32");
    bench_decode_dictionary(c, "base32");
}

fn bench_base32hex(c: &mut Criterion) {
    bench_encode_dictionary(c, "base32hex");
    bench_decode_dictionary(c, "base32hex");
}

fn bench_base16(c: &mut Criterion) {
    bench_encode_dictionary(c, "base16");
    bench_decode_dictionary(c, "base16");
}

fn bench_hex(c: &mut Criterion) {
    bench_encode_dictionary(c, "hex");
    bench_decode_dictionary(c, "hex");
}

fn bench_bioctal(c: &mut Criterion) {
    bench_encode_dictionary(c, "bioctal");
    bench_decode_dictionary(c, "bioctal");
}

fn bench_base32_geohash(c: &mut Criterion) {
    bench_encode_dictionary(c, "base32_geohash");
    bench_decode_dictionary(c, "base32_geohash");
}

fn bench_base32_zbase(c: &mut Criterion) {
    bench_encode_dictionary(c, "base32_zbase");
    bench_decode_dictionary(c, "base32_zbase");
}

fn bench_base256_matrix(c: &mut Criterion) {
    bench_encode_dictionary(c, "base256_matrix");
    bench_decode_dictionary(c, "base256_matrix");
}

fn bench_base58(c: &mut Criterion) {
    bench_encode_dictionary(c, "base58");
    bench_decode_dictionary(c, "base58");
}

fn bench_base85(c: &mut Criterion) {
    bench_encode_dictionary(c, "base85");
    bench_decode_dictionary(c, "base85");
}

fn bench_cards(c: &mut Criterion) {
    bench_encode_dictionary(c, "cards");
    bench_decode_dictionary(c, "cards");
}

fn bench_emoji_faces(c: &mut Criterion) {
    bench_encode_dictionary(c, "emoji_faces");
    bench_decode_dictionary(c, "emoji_faces");
}

/// Print platform info at the start
fn print_platform_info(_c: &mut Criterion) {
    let info = PlatformInfo::detect();
    eprintln!("\n╔══════════════════════════════════════════════════════════╗");
    eprintln!("║ base-d benchmark suite                                   ║");
    eprintln!("║ Platform: {:48} ║", info.display());
    eprintln!("╚══════════════════════════════════════════════════════════╝\n");
}

criterion_group!(
    name = platform_info;
    config = Criterion::default().sample_size(10);
    targets = print_platform_info
);

criterion_group!(
    name = rfc_encodings;
    config = Criterion::default();
    targets =
        bench_base64,
        bench_base64url,
        bench_base32,
        bench_base32hex,
        bench_base16,
        bench_hex,
        bench_bioctal,
        bench_base32_geohash,
        bench_base32_zbase
);

criterion_group!(
    name = high_density;
    config = Criterion::default();
    targets = bench_base256_matrix
);

criterion_group!(
    name = non_power_of_two;
    config = Criterion::default().sample_size(50);
    targets = bench_base58, bench_base85
);

criterion_group!(
    name = fun_encodings;
    config = Criterion::default().sample_size(50);
    targets = bench_cards, bench_emoji_faces
);

criterion_main!(
    platform_info,
    rfc_encodings,
    high_density,
    non_power_of_two,
    fun_encodings
);
