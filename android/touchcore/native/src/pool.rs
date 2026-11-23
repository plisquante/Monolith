//! Size-class slot pool with epoch-based deferred reclamation. No tracing and no stop-the-world: a retired slot
//! is reused after two epoch advances, which is when every reader that could still see it has finished.
//!
//! This keeps the call surface the touchcore core uses (`alloc_slot`, `retire`, `advance_and_reclaim`, counters).
//! It is not a line-by-line port of the Zig `SlabPool`: that one carried slab headers, conservative pointer lookup
//! and monitor release for the collector, which this runtime does not have.
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

/// Index of the smallest size class that holds `size` bytes, or `None` above `MAX_SMALL`.
pub fn size_class(size: usize) -> Option<usize> {
    SIZE_CLASSES.iter().position(|&c| c >= size)
}

struct Slab {
    base: NonNull<u8>,
}

impl Drop for Slab {
    fn drop(&mut self) {
        // SAFETY: allocated in `new_slab` with this exact layout.
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
    /// Slots ready for reuse, per size class.
    free: [Vec<NonNull<u8>>; NUM_SC],
    /// Slots retired in epoch `e` wait in `retired[e % EPOCHS]` until they are two epochs old.
    retired: [[Vec<NonNull<u8>>; NUM_SC]; EPOCHS],
    /// Backing memory; kept for the pool's lifetime.
    slabs: Vec<Slab>,
    epoch: u64,
    stats: Stats,
}

// SAFETY: the pool owns all of its memory; it is moved between threads, never shared.
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

    /// A slot of size class `sc`. `None` only when a new slab cannot be mapped.
    pub fn alloc_slot(&mut self, sc: usize) -> Option<NonNull<u8>> {
        if self.free[sc].is_empty() {
            self.new_slab(sc)?;
        }
        let p = self.free[sc].pop()?;
        self.stats.live_bytes += SIZE_CLASSES[sc];
        Some(p)
    }

    /// Hands a slot back. It stays unusable until `advance_and_reclaim` has moved it past two epochs.
    ///
    /// # Safety
    /// `p` must come from `alloc_slot(sc)` of this pool and must not be used after this call.
    pub unsafe fn retire(&mut self, p: NonNull<u8>, sc: usize) {
        self.stats.live_bytes -= SIZE_CLASSES[sc];
        self.retired[(self.epoch as usize) % EPOCHS][sc].push(p);
    }

    /// Ends the current epoch. Slots retired two epochs ago become reusable.
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
        // SAFETY: non-zero size, valid alignment.
        let base = NonNull::new(unsafe { alloc(layout) })?;
        self.slabs.push(Slab { base });
        self.stats.committed_bytes += SLAB_SIZE;
        let size = SIZE_CLASSES[sc];
        let n = SLAB_SIZE / size;
        // push in reverse so the first alloc returns the lowest address
        for i in (0..n).rev() {
            // SAFETY: `i * size + size <= n * size <= SLAB_SIZE`.
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
        // SAFETY: `p` came from `alloc_slot(sc)` and is not used again.
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
