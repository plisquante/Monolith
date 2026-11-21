//! Chunk düzeni. Boyutlar derleme zamanında sabittir; çalışma zamanında boyut kararı yoktur.
//!
//! Sayısal sabitlerin kaynağı: Minecraft Java 26.3 istemci jar'ı
//! (`data/minecraft/dimension_type/overworld.json`, `min_y = -64`, `height = 384`).
//! Bu sabitler jar'dan okunmaz, burada yazılıdır; kaynağı ARCHITECTURE.md'de.
#![forbid(unsafe_code)]

/// Overworld alt sınırı (`min_y`), 26.3 verisi.
pub const MIN_Y: i32 = -64;
/// Overworld yüksekliği (`height`), 26.3 verisi.
pub const WORLD_HEIGHT: usize = 384;
/// Overworld deniz seviyesi (`sea_level`), 26.3 verisi.
pub const SEA_LEVEL: i32 = 63;

/// Yatay chunk kenarı.
pub const CHUNK_SIZE: usize = 16;
/// Chunk başına blok sayısı: 16 x 384 x 16.
pub const CHUNK_BLOCKS: usize = CHUNK_SIZE * CHUNK_SIZE * WORLD_HEIGHT;

// Derleme zamanı düzen kilitleri: boyut değişirse derleme durur.
const _: () = assert!(CHUNK_BLOCKS == 98_304);
const _: () = assert!(WORLD_HEIGHT.is_multiple_of(CHUNK_SIZE));
const _: () = assert!(WORLD_HEIGHT / CHUNK_SIZE == 24);

/// Blok durum kimliği. Anlamı kayıt defterinden gelir (henüz üretilmedi).
///
/// `u16` sınırı 26.3 blok durum sayısına yeterli olup olmadığı kayıt defteri üretilene kadar
/// doğrulanmadı (ARCHITECTURE.md, risk 5).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockStateId(pub u16);

/// Tek bir chunk sütunu. Blok verisi sabit boyutlu bir dizidir; büyümez, küçülmez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    blocks: Box<[BlockStateId; CHUNK_BLOCKS]>,
}

impl Default for Chunk {
    fn default() -> Self {
        Self::filled(BlockStateId::default())
    }
}

impl Chunk {
    /// Tüm blokları `id` ile dolu bir chunk.
    #[must_use]
    pub fn filled(id: BlockStateId) -> Self {
        // Vec üzerinden kurulur: CHUNK_BLOCKS * 2 bayt, yığın (stack) üzerinde kopyalanmaz.
        let v: Vec<BlockStateId> = vec![id; CHUNK_BLOCKS];
        let blocks: Box<[BlockStateId; CHUNK_BLOCKS]> = v
            .into_boxed_slice()
            .try_into()
            .expect("uzunluk CHUNK_BLOCKS ile aynı");
        Self { blocks }
    }

    /// Dünya koordinatından yerel indeks. Chunk-yerel `x`, `z` in `0..16`, `world_y` in `MIN_Y..MIN_Y+384`.
    ///
    /// Düzen: `((y - MIN_Y) * 16 + z) * 16 + x`. Dışarıdaki koordinat `None` döner, panik yok.
    #[must_use]
    pub const fn index(x: usize, world_y: i32, z: usize) -> Option<usize> {
        if x >= CHUNK_SIZE || z >= CHUNK_SIZE {
            return None;
        }
        let rel = world_y as i64 - MIN_Y as i64;
        if rel < 0 || rel >= WORLD_HEIGHT as i64 {
            return None;
        }
        Some((rel as usize * CHUNK_SIZE + z) * CHUNK_SIZE + x)
    }

    /// Blok durumunu okur. Sınır dışıysa `None`.
    #[must_use]
    pub fn get(&self, x: usize, world_y: i32, z: usize) -> Option<BlockStateId> {
        Self::index(x, world_y, z).map(|i| self.blocks[i])
    }

    /// Blok durumunu yazar. Sınır dışıysa `false` döner ve hiçbir şey değişmez.
    pub fn set(&mut self, x: usize, world_y: i32, z: usize, id: BlockStateId) -> bool {
        match Self::index(x, world_y, z) {
            Some(i) => {
                self.blocks[i] = id;
                true
            }
            None => false,
        }
    }

    /// Ham blok dizisi (düzen sırasıyla). Serileştirme ve karşılaştırma için.
    #[must_use]
    pub fn as_slice(&self) -> &[BlockStateId] {
        &self.blocks[..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_constants() {
        assert_eq!(CHUNK_BLOCKS, 98_304);
        assert_eq!(WORLD_HEIGHT / CHUNK_SIZE, 24);
        assert!(SEA_LEVEL >= MIN_Y && SEA_LEVEL < MIN_Y + WORLD_HEIGHT as i32);
    }

    #[test]
    fn index_bounds() {
        assert_eq!(Chunk::index(0, MIN_Y, 0), Some(0));
        assert_eq!(Chunk::index(15, MIN_Y + 383, 15), Some(CHUNK_BLOCKS - 1));
        assert_eq!(Chunk::index(16, MIN_Y, 0), None);
        assert_eq!(Chunk::index(0, MIN_Y, 16), None);
        assert_eq!(Chunk::index(0, MIN_Y - 1, 0), None);
        assert_eq!(Chunk::index(0, MIN_Y + 384, 0), None);
    }

    #[test]
    fn index_is_bijective() {
        let mut seen = vec![false; CHUNK_BLOCKS];
        for y in MIN_Y..MIN_Y + WORLD_HEIGHT as i32 {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let i = Chunk::index(x, y, z).unwrap();
                    assert!(!seen[i], "indeks çakıştı: {i}");
                    seen[i] = true;
                }
            }
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn get_set_roundtrip_and_out_of_range() {
        let mut c = Chunk::default();
        assert_eq!(c.get(3, 64, 9), Some(BlockStateId(0)));
        assert!(c.set(3, 64, 9, BlockStateId(42)));
        assert_eq!(c.get(3, 64, 9), Some(BlockStateId(42)));
        assert_eq!(c.as_slice().iter().filter(|b| b.0 == 42).count(), 1);

        assert!(!c.set(16, 64, 0, BlockStateId(1)));
        assert_eq!(c.get(16, 64, 0), None);
        assert_eq!(c.as_slice().iter().filter(|b| b.0 == 1).count(), 0);
    }

    #[test]
    fn filled_sets_every_block() {
        let c = Chunk::filled(BlockStateId(7));
        assert_eq!(c.as_slice().len(), CHUNK_BLOCKS);
        assert!(c.as_slice().iter().all(|b| b.0 == 7));
    }
}
