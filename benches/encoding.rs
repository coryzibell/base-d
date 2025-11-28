use base_d::{decode, encode, Dictionary, DictionaryRegistry, EncodingMode};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn get_dictionary(name: &str) -> Dictionary {
    let config = DictionaryRegistry::load_default().unwrap();
    let dictionary_config = config.get_dictionary(name).unwrap();

    match dictionary_config.mode {
        EncodingMode::ByteRange => {
            let start = dictionary_config.start_codepoint.unwrap();
            Dictionary::new_with_mode_and_range(
                Vec::new(),
                dictionary_config.mode.clone(),
                None,
                Some(start),
            )
            .unwrap()
        }
        _ => {
            let chars: Vec<char> = dictionary_config.chars.chars().collect();
            let padding = dictionary_config
                .padding
                .as_ref()
                .and_then(|s| s.chars().next());
            Dictionary::new_with_mode(chars, dictionary_config.mode.clone(), padding).unwrap()
        }
    }
}

fn bench_encode_base64(c: &mut Criterion) {
    let dictionary = get_dictionary("base64");
    let mut group = c.benchmark_group("encode_base64");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| encode(black_box(data), black_box(&dictionary)));
        });
    }
    group.finish();
}

fn bench_decode_base64(c: &mut Criterion) {
    let dictionary = get_dictionary("base64");
    let mut group = c.benchmark_group("decode_base64");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
        let encoded = encode(&data, &dictionary);

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &encoded, |b, encoded| {
            b.iter(|| decode(black_box(encoded), black_box(&dictionary)).unwrap());
        });
    }
    group.finish();
}

fn bench_encode_base32(c: &mut Criterion) {
    let dictionary = get_dictionary("base32");
    let mut group = c.benchmark_group("encode_base32");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| encode(black_box(data), black_box(&dictionary)));
        });
    }
    group.finish();
}

fn bench_encode_base100(c: &mut Criterion) {
    let dictionary = get_dictionary("base100");
    let mut group = c.benchmark_group("encode_base100");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| encode(black_box(data), black_box(&dictionary)));
        });
    }
    group.finish();
}

fn bench_decode_base100(c: &mut Criterion) {
    let dictionary = get_dictionary("base100");
    let mut group = c.benchmark_group("decode_base100");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
        let encoded = encode(&data, &dictionary);

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &encoded, |b, encoded| {
            b.iter(|| decode(black_box(encoded), black_box(&dictionary)).unwrap());
        });
    }
    group.finish();
}

fn bench_encode_hex(c: &mut Criterion) {
    let dictionary = get_dictionary("hex");
    let mut group = c.benchmark_group("encode_hex");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| encode(black_box(data), black_box(&dictionary)));
        });
    }
    group.finish();
}

fn bench_encode_base1024(c: &mut Criterion) {
    let dictionary = get_dictionary("base1024");
    let mut group = c.benchmark_group("encode_base1024");

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| encode(black_box(data), black_box(&dictionary)));
        });
    }
    group.finish();
}

fn bench_decode_base1024(c: &mut Criterion) {
    let dictionary = get_dictionary("base1024");
    let mut group = c.benchmark_group("decode_base1024");

    for size in [64, 256, 1024, 4096].iter() {
        let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
        let encoded = encode(&data, &dictionary);

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &encoded, |b, encoded| {
            b.iter(|| decode(black_box(encoded), black_box(&dictionary)).unwrap());
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_encode_base64,
    bench_decode_base64,
    bench_encode_base32,
    bench_encode_base100,
    bench_decode_base100,
    bench_encode_hex,
    bench_encode_base1024,
    bench_decode_base1024,
);
criterion_main!(benches);
