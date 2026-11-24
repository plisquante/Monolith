//! Çağ tabanlı ertelenmiş geri kazanımlı boyut sınıfı yuva havuzu. İzleme yok, durdur-dünya yok: emekli edilmiş bir yuva
//! iki çağ ilerlemesinden sonra yeniden kullanılır; bu, onu hâlâ görebilecek her okuyucunun bitirdiği andır.
//!
//! Bu, touchcore çekirdeğinin kullandığı çağrı yüzeyini korur (`alloc_slot`, `retire`, `advance_and_reclaim`, sayaçlar).
//! Zig `SlabPool`'un satır satır bir taşıması değildir: o, bu çalışma zamanında olmayan toplayıcı için slab başlıkları,
//! muhafazakâr işaretçi araması ve monitör serbest bırakma taşıyordu.
use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

pub const SIZE_CLASSES: [usize; 35] = [
    16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 448, 512, 640, 768, 896, 1024, 1280,
    1536, 2048, 2560, 3072, 4096, 6144, 8192, 12288, 16384, 24576, 32768,
];
pub const NUM_SC: usize = SIZE_CLASSES.len();
pub const MAX_SMALL: usize = SIZE_CLASSES[NUM_SC - 1];
pub const SLAB_SIZE: usize = 128 * 1024;
const SLAB_ALIGN: usize = 4096;
const EPOCHS: usize = 3;

/// `size` bayt tutan en küçük boyut sınıfının indeksi, ya da `MAX_SMALL`'ın üstünde `None`.
pub fn size_class(size: usize) -> Option<usize> {
    SIZE_CLASSES.iter().position(|&c| c >= size)
}

struct Slab {
    base: NonNull<u8>,
}

impl Drop for Slab {
    fn drop(&mut self) {
        // GÜVENLİK: `new_slab` içinde tam olarak bu düzenle tahsis edildi.
        unsafe { dealloc(self.base.as_ptr(), Layout::from_size_align(SLAB_SIZE, SLAB_ALIGN).expect("slab layout")) };
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub committed_bytes: usize,
    pub live_bytes: usize,
    pub reclaimed_bytes: usize,
}

pub struct SlabPool {
    /// Yeniden kullanıma hazır yuvalar, boyut sınıfı başına.
    free: [Vec<NonNull<u8>>; NUM_SC],
    /// `e` çağında emekli edilen yuvalar, iki çağ yaşına gelene kadar `retired[e % EPOCHS]` içinde bekler.
    retired: [[Vec<NonNull<u8>>; NUM_SC]; EPOCHS],
    /// Destek belleği; havuzun ömrü boyunca tutulur.
    slabs: Vec<Slab>,
    epoch: u64,
    stats: Stats,
}

// GÜVENLİK: havuz tüm belleğinin sahibidir; iş parçacıkları arasında taşınır, asla paylaşılmaz.
unsafe impl Send for SlabPool {}

impl Default for SlabPool {
    fn default() -> Self {
        Self::new()
    }
}

impl SlabPool {
    pub fn new() -> Self {
        SlabPool {
            free: std::array::from_fn(|_| Vec::new()),
            retired: std::array::from_fn(|_| std::array::from_fn(|_| Vec::new())),
            slabs: Vec::new(),
            epoch: 0,
            stats: Stats::default(),
        }
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// `sc` boyut sınıfından bir yuva. `None` yalnızca yeni bir slab eşlenemediğinde.
    pub fn alloc_slot(&mut self, sc: usize) -> Option<NonNull<u8>> {
        if self.free[sc].is_empty() {
            self.new_slab(sc)?;
        }
        let p = self.free[sc].pop()?;
        self.stats.live_bytes += SIZE_CLASSES[sc];
        Some(p)
    }

    /// Bir yuvayı geri verir. `advance_and_reclaim` onu iki çağ ilerletene kadar kullanılamaz kalır.
    ///
    /// # Safety
    /// `p`, bu havuzun `alloc_slot(sc)` çağrısından gelmeli ve bu çağrıdan sonra kullanılmamalıdır.
    pub unsafe fn retire(&mut self, p: NonNull<u8>, sc: usize) {
        self.stats.live_bytes -= SIZE_CLASSES[sc];
        self.retired[(self.epoch as usize) % EPOCHS][sc].push(p);
    }

    /// Mevcut çağı bitirir. İki çağ önce emekli edilen yuvalar yeniden kullanılabilir olur.
    pub fn advance_and_reclaim(&mut self) {
        self.epoch += 1;
        let bucket = (self.epoch as usize + 1) % EPOCHS; // the epoch that is now two behind
        for (sc, (retired, free)) in self.retired[bucket].iter_mut().zip(self.free.iter_mut()).enumerate() {
            let slots = std::mem::take(retired);
            self.stats.reclaimed_bytes += slots.len() * SIZE_CLASSES[sc];
            free.extend(slots);
        }
    }

    fn new_slab(&mut self, sc: usize) -> Option<()> {
        let layout = Layout::from_size_align(SLAB_SIZE, SLAB_ALIGN).ok()?;
        // GÜVENLİK: sıfırdan farklı boyut, geçerli hizalama.
        let base = NonNull::new(unsafe { alloc(layout) })?;
        self.slabs.push(Slab { base });
        self.stats.committed_bytes += SLAB_SIZE;
        let size = SIZE_CLASSES[sc];
        let n = SLAB_SIZE / size;
        // ters sırada it, böylece ilk alloc en düşük adresi döndürür
        for i in (0..n).rev() {
            // GÜVENLİK: `i * size + size <= n * size <= SLAB_SIZE`.
            let p = unsafe { NonNull::new_unchecked(base.as_ptr().add(i * size)) };
            self.free[sc].push(p);
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_classes_round_up() {
        assert_eq!(size_class(1), Some(0));
        assert_eq!(size_class(64), Some(6));
        assert_eq!(size_class(65), Some(7));
        assert_eq!(size_class(MAX_SMALL + 1), None);
    }

    #[test]
    fn retired_slot_is_reused_only_after_two_epochs() {
        let mut pool = SlabPool::new();
        let sc = size_class(64).unwrap();
        let p = pool.alloc_slot(sc).unwrap();
        // GÜVENLİK: `p`, `alloc_slot(sc)`'den geldi ve bir daha kullanılmaz.
        unsafe { pool.retire(p, sc) };
        pool.advance_and_reclaim();
        assert_ne!(pool.alloc_slot(sc).unwrap(), p, "one epoch is not enough");
        pool.advance_and_reclaim();
        assert_eq!(pool.stats().reclaimed_bytes, 64);
    }

    #[test]
    fn counters_track_commit_and_live_bytes() {
        let mut pool = SlabPool::new();
        let sc = size_class(64).unwrap();
        let _a = pool.alloc_slot(sc).unwrap();
        let st = pool.stats();
        assert_eq!(st.committed_bytes, SLAB_SIZE);
        assert_eq!(st.live_bytes, 64);
    }
}
