//! Ids of derived entities. They are computed from the owning
//! invocation instead of being random, so a re-run of the same
//! invocation over the same graph reproduces them. Derived ids are
//! UUIDv8; input ids are random UUIDv4, so the version tells the two
//! layers apart.

use uuid::Uuid;

use common::*;

/// UUID version of derived ids.
const DERIVED_VERSION: usize = 8;

/// Whether `id` belongs to a derived entity.
pub(crate) fn is_derived(id: &EntityId) -> bool {
    id.get_version_num() == DERIVED_VERSION
}

/// What tells apart the entities one run of an invocation creates.
pub(crate) enum Local<'a> {
    /// The `n`-th node created in the run.
    Node(u64),
    /// The `n`-th edge from `source` to `target` created in the run.
    Edge(&'a Pointee, &'a Pointee, u64),
    /// The `n`-th hyperedge created in the run.
    Hyperedge(u64),
}

/// Id of the entity `local` created by the invocation of `view` for
/// `key`.
pub(crate) fn derive_id(view: &str, key: Option<&EntityId>, local: Local<'_>) -> EntityId {
    let mut h = Fnv128::new();
    h.str(view);
    match key {
        None => h.byte(0),
        Some(key) => {
            h.byte(1);
            h.bytes(key.as_bytes());
        }
    }
    match local {
        Local::Node(n) => {
            h.byte(0);
            h.u64(n);
        }
        Local::Edge(source, target, n) => {
            h.byte(1);
            h.pointee(source);
            h.pointee(target);
            h.u64(n);
        }
        Local::Hyperedge(n) => {
            h.byte(2);
            h.u64(n);
        }
    }

    let mut bytes = h.finish().to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | ((DERIVED_VERSION as u8) << 4);
    // RFC 4122 variant.
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// 128-bit FNV-1a with a final mix. Unlike `DefaultHasher` it is
/// stable across platforms and Rust versions.
struct Fnv128(u128);

impl Fnv128 {
    const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
    const PRIME: u128 = 0x0000000001000000000000000000013b;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn byte(&mut self, b: u8) {
        self.0 ^= b as u128;
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn bytes(&mut self, bs: &[u8]) {
        for &b in bs {
            self.byte(b);
        }
    }

    fn u64(&mut self, n: u64) {
        self.bytes(&n.to_le_bytes());
    }

    fn str(&mut self, s: &str) {
        self.u64(s.len() as u64);
        self.bytes(s.as_bytes());
    }

    fn pointee(&mut self, p: &Pointee) {
        match p {
            Pointee::EntityId(id) => {
                self.byte(0);
                self.bytes(id.as_bytes());
            }
            Pointee::Path(path) => {
                self.byte(1);
                self.bytes(path.entity().as_bytes());
                self.str(path.local().as_str());
            }
        }
    }

    /// FNV's multiplication only carries low bits upwards; fold the
    /// high half back down so every output bit depends on the input.
    fn finish(&self) -> u128 {
        let mut x = self.0;
        x ^= x >> 64;
        x = x.wrapping_mul(0x9e3779b97f4a7c15f39cc0605cedc835);
        x ^= x >> 67;
        x = x.wrapping_mul(0xd6e8feb86659fd93a5b35705ed8f4a1b);
        x ^= x >> 61;
        x
    }
}
