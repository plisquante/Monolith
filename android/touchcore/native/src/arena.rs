//! Bump allocator over page-aligned chunks with O(1) scoped rewind. Port of `ArenaAllocator` in `mem_pool.zig`.
//! Nothing here is traced or collected: memory is valid until the arena is rewound, reset or dropped.
//! Chunks that end up empty are kept warm (up to `max_retained`) so the next frame does not hit the OS.
use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

pub const DEFAULT_CHUNK_SIZE: usize = 256 * 1024;
const PAGE: usize = 4096;
/// Debug builds overwrite rewound memory so a dangling pointer into a rewound scope fails loudly.
const POISON: u8 = 0xAA;

struct Chunk {
    id: u64,
    base: NonNull<u8>,
    cap: usize,
    used: usize,
}

impl Chunk {
    fn layout(cap: usize) -> Layout {
        Layout::from_size_align(cap, PAGE).expect("chunk layout")
    }

    fn poison(&mut self, from: usize, to: usize) {
        if cfg!(debug_assertions) && to > from {
            // SAFETY: `from..to` lies inside this chunk's allocation (`to <= used <= cap`).
            unsafe { std::ptr::write_bytes(self.base.as_ptr().add(from), POISON, to - from) };
        }
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        // SAFETY: allocated in `Arena::alloc_slow` with exactly this layout.
        unsafe { dealloc(self.base.as_ptr(), Chunk::layout(self.cap)) };
    }
}

/// A rewind point. Valid only while the arena is not reset past it (scope discipline, as in the Zig original).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    top: Option<u64>,
    used: usize,
    total: usize,
}

pub struct Arena {
    /// Chunks in use; the last one is the current bump chunk.
    chain: Vec<Chunk>,
    /// Empty chunks of the standard size, kept for reuse.
    free: Vec<Chunk>,
    next_id: u64,
    chunk_size: usize,
    max_retained: usize,
    total_allocated: usize,
    committed_bytes: usize,
}

// SAFETY: an arena owns its memory exclusively; handing it to another thread moves that ownership.
unsafe impl Send for Arena {}

impl Arena {
    pub fn new(chunk_size: usize, max_retained: usize) -> Self {
        Arena {
            chain: Vec::new(),
            free: Vec::new(),
            next_id: 1,
            chunk_size: chunk_size.max(PAGE),
            max_retained,
            total_allocated: 0,
            committed_bytes: 0,
        }
    }

    pub fn total_allocated(&self) -> usize {
        self.total_allocated
    }

    pub fn committed_bytes(&self) -> usize {
        self.committed_bytes
    }

    /// Allocates `size` bytes aligned to `align` (at most a page). `None` if the request cannot be honoured.
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        let a = align.max(8);
        if a > PAGE {
            return None;
        }
        if let Some(ch) = self.chain.last_mut() {
            let off = (ch.used + a - 1) & !(a - 1);
            if off <= ch.cap && size <= ch.cap - off {
                // SAFETY: `off + size <= cap`, so the pointer stays inside the chunk.
                let p = unsafe { NonNull::new_unchecked(ch.base.as_ptr().add(off)) };
                ch.used = off + size;
                self.total_allocated += size;
                return Some(p);
            }
        }
        self.alloc_slow(size, a)
    }

    pub fn alloc_zeroed(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        let p = self.alloc(size, align)?;
        // SAFETY: `alloc` returned `size` writable bytes at `p`.
        unsafe { std::ptr::write_bytes(p.as_ptr(), 0, size) };
        Some(p)
    }

    #[cold]
    fn alloc_slow(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        if size > usize::MAX / 2 {
            return None;
        }
        let need = self.chunk_size.max((size + align + PAGE - 1) & !(PAGE - 1));
        let mut chunk = match self.free.iter().rposition(|c| c.cap >= need) {
            Some(i) => {
                let mut c = self.free.remove(i);
                c.used = 0;
                c
            }
            None => {
                let layout = Chunk::layout(need);
                // SAFETY: `layout` has non-zero size (need >= PAGE).
                let raw = unsafe { alloc(layout) };
                let base = NonNull::new(raw)?;
                self.committed_bytes += need;
                let id = self.next_id;
                self.next_id += 1;
                Chunk { id, base, cap: need, used: 0 }
            }
        };
        // a fresh or recycled chunk starts at offset 0, which is aligned for every request accepted here
        // SAFETY: `need >= size`, so the first `size` bytes are inside the chunk.
        let p = unsafe { NonNull::new_unchecked(chunk.base.as_ptr()) };
        chunk.used = size;
        self.total_allocated += size;
        self.chain.push(chunk);
        Some(p)
    }

    pub fn mark(&self) -> Mark {
        let top = self.chain.last();
        Mark { top: top.map(|c| c.id), used: top.map_or(0, |c| c.used), total: self.total_allocated }
    }

    /// Rewinds to `m`, recycling every chunk opened after it.
    pub fn restore(&mut self, m: Mark) {
        while self.chain.last().map(|c| c.id) != m.top {
            match self.chain.pop() {
                Some(ch) => self.recycle_or_release(ch),
                None => break,
            }
        }
        if let Some(c) = self.chain.last_mut() {
            if c.used > m.used {
                let used = c.used;
                c.poison(m.used, used);
            }
            c.used = m.used;
        }
        self.total_allocated = m.total;
    }

    /// Rewinds everything in O(1) time per chunk, keeping one base chunk warm for the next frame. An oversized
    /// base chunk (made for one large request) is released so it cannot pin memory forever.
    pub fn reset_retaining_capacity(&mut self) {
        if self.chain.is_empty() {
            self.total_allocated = 0;
            return;
        }
        while self.chain.len() > 1 {
            let ch = self.chain.pop().expect("len > 1");
            self.recycle_or_release(ch);
        }
        let base = self.chain.last_mut().expect("non-empty");
        if base.cap > self.chunk_size {
            let ch = self.chain.pop().expect("non-empty");
            self.committed_bytes -= ch.cap;
        } else {
            let used = base.used;
            base.poison(0, used);
            base.used = 0;
        }
        self.total_allocated = 0;
    }

    fn recycle_or_release(&mut self, mut ch: Chunk) {
        let used = ch.used;
        ch.poison(0, used);
        ch.used = 0;
        if ch.cap == self.chunk_size && self.free.len() < self.max_retained {
            self.free.push(ch);
        } else {
            self.committed_bytes -= ch.cap;
            drop(ch);
        }
    }
}

impl Default for Arena {
    fn default() -> Self {
        Arena::new(DEFAULT_CHUNK_SIZE, 8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_allocations_are_aligned_and_disjoint() {
        let mut a = Arena::new(64 * 1024, 2);
        let p1 = a.alloc(24, 16).unwrap().as_ptr() as usize;
        let p2 = a.alloc(40, 64).unwrap().as_ptr() as usize;
        assert_eq!(p1 % 16, 0);
        assert_eq!(p2 % 64, 0);
        assert!(p2 >= p1 + 24);
        assert_eq!(a.total_allocated(), 64);
    }

    #[test]
    fn restore_rewinds_across_chunks_and_reuses_them() {
        let mut a = Arena::new(4096, 4);
        let _keep = a.alloc(100, 8).unwrap();
        let m = a.mark();
        for _ in 0..10 {
            a.alloc(1000, 8).unwrap();
        }
        let committed = a.committed_bytes();
        a.restore(m);
        assert_eq!(a.total_allocated(), 100);
        // the chunks opened after the mark went to the free list, so a new burst commits nothing
        for _ in 0..10 {
            a.alloc(1000, 8).unwrap();
        }
        assert_eq!(a.committed_bytes(), committed);
    }

    #[test]
    fn oversized_base_chunk_is_released_on_reset() {
        let mut a = Arena::new(4096, 2);
        a.alloc(20_000, 8).unwrap();
        assert!(a.committed_bytes() >= 20_000);
        a.reset_retaining_capacity();
        assert_eq!(a.committed_bytes(), 0);
        assert_eq!(a.total_allocated(), 0);
    }

    #[test]
    fn alignment_beyond_a_page_is_refused() {
        let mut a = Arena::default();
        assert!(a.alloc(8, 8192).is_none());
    }
}
