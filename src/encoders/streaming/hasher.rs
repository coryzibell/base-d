use crate::features::hashing::HashAlgorithm;

// Helper for managing hash state during streaming
#[allow(clippy::large_enum_variant)]
pub(super) enum HasherWriter {
    Md5(md5::Md5),
    Sha224(sha2::Sha224),
    Sha256(sha2::Sha256),
    Sha384(sha2::Sha384),
    Sha512(sha2::Sha512),
    Sha3_224(sha3::Sha3_224),
    Sha3_256(sha3::Sha3_256),
    Sha3_384(sha3::Sha3_384),
    Sha3_512(sha3::Sha3_512),
    Keccak224(sha3::Keccak224),
    Keccak256(sha3::Keccak256),
    Keccak384(sha3::Keccak384),
    Keccak512(sha3::Keccak512),
    Blake2b(blake2::Blake2b512),
    Blake2s(blake2::Blake2s256),
    Blake3(blake3::Hasher),
    Crc16(Box<crc::Digest<'static, u16>>),
    Crc32(Box<crc::Digest<'static, u32>>),
    Crc32c(Box<crc::Digest<'static, u32>>),
    Crc64(Box<crc::Digest<'static, u64>>),
    XxHash32(twox_hash::XxHash32),
    XxHash64(twox_hash::XxHash64),
    XxHash3_64(twox_hash::xxhash3_64::Hasher),
    XxHash3_128(twox_hash::xxhash3_128::Hasher),
    Ascon(ascon_hash::AsconHash256),
    K12(k12::KangarooTwelve),
}

impl HasherWriter {
    pub(super) fn update(&mut self, data: &[u8]) {
        use sha2::Digest;
        use std::hash::Hasher;

        match self {
            HasherWriter::Md5(h) => {
                h.update(data);
            }
            HasherWriter::Sha224(h) => {
                h.update(data);
            }
            HasherWriter::Sha256(h) => {
                h.update(data);
            }
            HasherWriter::Sha384(h) => {
                h.update(data);
            }
            HasherWriter::Sha512(h) => {
                h.update(data);
            }
            HasherWriter::Sha3_224(h) => {
                h.update(data);
            }
            HasherWriter::Sha3_256(h) => {
                h.update(data);
            }
            HasherWriter::Sha3_384(h) => {
                h.update(data);
            }
            HasherWriter::Sha3_512(h) => {
                h.update(data);
            }
            HasherWriter::Keccak224(h) => {
                h.update(data);
            }
            HasherWriter::Keccak256(h) => {
                h.update(data);
            }
            HasherWriter::Keccak384(h) => {
                h.update(data);
            }
            HasherWriter::Keccak512(h) => {
                h.update(data);
            }
            HasherWriter::Blake2b(h) => {
                h.update(data);
            }
            HasherWriter::Blake2s(h) => {
                h.update(data);
            }
            HasherWriter::Blake3(h) => {
                h.update(data);
            }
            HasherWriter::Crc16(digest) => {
                digest.update(data);
            }
            HasherWriter::Crc32(digest) => {
                digest.update(data);
            }
            HasherWriter::Crc32c(digest) => {
                digest.update(data);
            }
            HasherWriter::Crc64(digest) => {
                digest.update(data);
            }
            HasherWriter::XxHash32(h) => {
                h.write(data);
            }
            HasherWriter::XxHash64(h) => {
                h.write(data);
            }
            HasherWriter::XxHash3_64(h) => {
                h.write(data);
            }
            HasherWriter::XxHash3_128(h) => {
                h.write(data);
            }
            HasherWriter::Ascon(h) => {
                use ascon_hash::Digest as AsconDigest;
                h.update(data);
            }
            HasherWriter::K12(h) => {
                use k12::digest::Update;
                h.update(data);
            }
        }
    }

    pub(super) fn finalize(self) -> Vec<u8> {
        use sha2::Digest;
        use std::hash::Hasher;

        match self {
            HasherWriter::Md5(h) => h.finalize().to_vec(),
            HasherWriter::Sha224(h) => h.finalize().to_vec(),
            HasherWriter::Sha256(h) => h.finalize().to_vec(),
            HasherWriter::Sha384(h) => h.finalize().to_vec(),
            HasherWriter::Sha512(h) => h.finalize().to_vec(),
            HasherWriter::Sha3_224(h) => h.finalize().to_vec(),
            HasherWriter::Sha3_256(h) => h.finalize().to_vec(),
            HasherWriter::Sha3_384(h) => h.finalize().to_vec(),
            HasherWriter::Sha3_512(h) => h.finalize().to_vec(),
            HasherWriter::Keccak224(h) => h.finalize().to_vec(),
            HasherWriter::Keccak256(h) => h.finalize().to_vec(),
            HasherWriter::Keccak384(h) => h.finalize().to_vec(),
            HasherWriter::Keccak512(h) => h.finalize().to_vec(),
            HasherWriter::Blake2b(h) => h.finalize().to_vec(),
            HasherWriter::Blake2s(h) => h.finalize().to_vec(),
            HasherWriter::Blake3(h) => h.finalize().as_bytes().to_vec(),
            HasherWriter::Crc16(digest) => digest.finalize().to_be_bytes().to_vec(),
            HasherWriter::Crc32(digest) => digest.finalize().to_be_bytes().to_vec(),
            HasherWriter::Crc32c(digest) => digest.finalize().to_be_bytes().to_vec(),
            HasherWriter::Crc64(digest) => digest.finalize().to_be_bytes().to_vec(),
            HasherWriter::XxHash32(h) => (h.finish() as u32).to_be_bytes().to_vec(),
            HasherWriter::XxHash64(h) => h.finish().to_be_bytes().to_vec(),
            HasherWriter::XxHash3_64(h) => h.finish().to_be_bytes().to_vec(),
            HasherWriter::XxHash3_128(h) => {
                let hash = h.finish_128();
                let mut result = Vec::with_capacity(16);
                result.extend_from_slice(&hash.to_be_bytes());
                result
            }
            HasherWriter::Ascon(h) => {
                use ascon_hash::Digest as AsconDigest;
                h.finalize().to_vec()
            }
            HasherWriter::K12(h) => {
                use k12::digest::ExtendableOutput;
                use k12::digest::XofReader;
                let mut reader = h.finalize_xof();
                let mut output = vec![0u8; 32];
                reader.read(&mut output);
                output
            }
        }
    }
}

pub(super) fn create_hasher_writer(
    algo: HashAlgorithm,
    config: &crate::features::hashing::XxHashConfig,
) -> HasherWriter {
    use sha2::Digest;

    match algo {
        HashAlgorithm::Md5 => HasherWriter::Md5(md5::Md5::new()),
        HashAlgorithm::Sha224 => HasherWriter::Sha224(sha2::Sha224::new()),
        HashAlgorithm::Sha256 => HasherWriter::Sha256(sha2::Sha256::new()),
        HashAlgorithm::Sha384 => HasherWriter::Sha384(sha2::Sha384::new()),
        HashAlgorithm::Sha512 => HasherWriter::Sha512(sha2::Sha512::new()),
        HashAlgorithm::Sha3_224 => HasherWriter::Sha3_224(sha3::Sha3_224::new()),
        HashAlgorithm::Sha3_256 => HasherWriter::Sha3_256(sha3::Sha3_256::new()),
        HashAlgorithm::Sha3_384 => HasherWriter::Sha3_384(sha3::Sha3_384::new()),
        HashAlgorithm::Sha3_512 => HasherWriter::Sha3_512(sha3::Sha3_512::new()),
        HashAlgorithm::Keccak224 => HasherWriter::Keccak224(sha3::Keccak224::new()),
        HashAlgorithm::Keccak256 => HasherWriter::Keccak256(sha3::Keccak256::new()),
        HashAlgorithm::Keccak384 => HasherWriter::Keccak384(sha3::Keccak384::new()),
        HashAlgorithm::Keccak512 => HasherWriter::Keccak512(sha3::Keccak512::new()),
        HashAlgorithm::Blake2b => HasherWriter::Blake2b(blake2::Blake2b512::new()),
        HashAlgorithm::Blake2s => HasherWriter::Blake2s(blake2::Blake2s256::new()),
        HashAlgorithm::Blake3 => HasherWriter::Blake3(blake3::Hasher::new()),
        HashAlgorithm::Crc16 => {
            static CRC: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_IBM_SDLC);
            HasherWriter::Crc16(Box::new(CRC.digest()))
        }
        HashAlgorithm::Crc32 => {
            static CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
            HasherWriter::Crc32(Box::new(CRC.digest()))
        }
        HashAlgorithm::Crc32c => {
            static CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);
            HasherWriter::Crc32c(Box::new(CRC.digest()))
        }
        HashAlgorithm::Crc64 => {
            static CRC: crc::Crc<u64> = crc::Crc::<u64>::new(&crc::CRC_64_ECMA_182);
            HasherWriter::Crc64(Box::new(CRC.digest()))
        }
        HashAlgorithm::XxHash32 => {
            HasherWriter::XxHash32(twox_hash::XxHash32::with_seed(config.seed as u32))
        }
        HashAlgorithm::XxHash64 => {
            HasherWriter::XxHash64(twox_hash::XxHash64::with_seed(config.seed))
        }
        HashAlgorithm::XxHash3_64 => {
            if let Some(ref secret) = config.secret {
                HasherWriter::XxHash3_64(
                    twox_hash::xxhash3_64::Hasher::with_seed_and_secret(
                        config.seed,
                        secret.as_slice(),
                    )
                    .expect(
                        "XXH3 secret validation should have been done in XxHashConfig::with_secret",
                    ),
                )
            } else {
                HasherWriter::XxHash3_64(twox_hash::xxhash3_64::Hasher::with_seed(config.seed))
            }
        }
        HashAlgorithm::XxHash3_128 => {
            if let Some(ref secret) = config.secret {
                HasherWriter::XxHash3_128(
                    twox_hash::xxhash3_128::Hasher::with_seed_and_secret(
                        config.seed,
                        secret.as_slice(),
                    )
                    .expect(
                        "XXH3 secret validation should have been done in XxHashConfig::with_secret",
                    ),
                )
            } else {
                HasherWriter::XxHash3_128(twox_hash::xxhash3_128::Hasher::with_seed(config.seed))
            }
        }
        HashAlgorithm::Ascon => {
            use ascon_hash::Digest;
            HasherWriter::Ascon(ascon_hash::AsconHash256::new())
        }
        HashAlgorithm::K12 => HasherWriter::K12(k12::KangarooTwelve::new()),
    }
}
