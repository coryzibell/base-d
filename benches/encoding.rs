use base_d::{decode, encode, Dictionary, DictionaryRegistry, EncodingMode};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

fn get_dictionary(name: &str) -> Dictionary {
    let config = DictionaryRegistry::load_default().unwrap();
    let dictionary_config = config.get_dictionary(name).unwrap();

    match dictionary_config.mode {
        EncodingMode::ByteRange => {
            let start = dictionary_config.start_codepoint.unwrap();
            Dictionary::builder()
                .chars(Vec::new())
                .mode(dictionary_config.mode.clone())
                .start_codepoint(start)
                .build()
                .unwrap()
        }
        _ => {
            let chars: Vec<char> = dictionary_config.chars.chars().collect();
            let padding = dictionary_config
                .padding
                .as_ref()
                .and_then(|s| s.chars().next());
            let mut builder = Dictionary::builder()
                .chars(chars)
                .mode(dictionary_config.mode.clone());
            if let Some(pad) = padding {
                builder = builder.padding(pad);
            }
            builder.build().unwrap()
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
