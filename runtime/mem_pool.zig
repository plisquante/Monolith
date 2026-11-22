//! Zero-stutter GC-free memory management:
//!   1. `ArenaAllocator`: lock-free page-backed bump allocator with `ArenaMark` O(1) save/restore
//!      and pooled empty-chunk recycling (warm chunks retained up to a cap, rest released to the OS).
//!   2. `SlabPool`: segregated size-class pool (`16..32768` B) combining lock-free per-thread bump
//!      slabs, thread-local caches, and quiescent 3-epoch deferred reclamation (zero STW pauses).
//!   3. Direct `std.mem.Allocator` adapter so Zig subsystems share the exact same zero-GC substrate.

const std = @import("std");

pub const DEFAULT_CHUNK_SIZE: usize = 256 * 1024;
pub const SIZE_CLASSES = [_]u32{
    16,    24,    32,    40,    48,    56,    64,    80,    96,    112,   128,  160,
    192,   224,   256,   320,   384,   448,   512,   640,   768,   896,   1024, 1280,
    1536,  2048,  2560,  3072,  4096,  6144,  8192,  12288, 16384, 24576, 32768,
};
pub const NUM_SC: usize = SIZE_CLASSES.len;
pub const MAX_SMALL: usize = SIZE_CLASSES[NUM_SC - 1];

pub const sc_lookup = blk: {
    @setEvalBranchQuota(40000);
    var t: [MAX_SMALL / 8 + 1]u8 = undefined;
    var c: usize = 0;
    for (&t, 0..) |*e, i| {
        while (SIZE_CLASSES[c] < i * 8) c += 1;
        e.* = @intCast(c);
    }
    break :blk t;
};

pub inline fn sizeClass(size: usize) usize {
    return sc_lookup[(size + 7) / 8];
}

pub inline fn alignUp(val: usize, align_bytes: usize) usize {
    return (val + align_bytes - 1) & ~(align_bytes - 1);
}

pub const Chunk = struct {
    ptr: [*]align(4096) u8,
    cap: usize,
    used: usize = 0,
    prev: ?*Chunk = null,
};

pub const ArenaMark = struct {
    chunk: ?*Chunk,
    used: usize,
    total_bytes: usize,
};

/// Fast linear bump allocator backed by page-aligned chunks with O(1) scoped rewind/reset.
/// Never triggers GC and never fragments small temporary allocations.
pub const ArenaAllocator = struct {
    current: ?*Chunk = null,
    free_chunks: ?*Chunk = null,
    n_free_chunks: usize = 0,
    max_retained_chunks: usize = 8,
    chunk_size: usize = DEFAULT_CHUNK_SIZE,
    total_allocated: usize = 0,
    committed_bytes: usize = 0,

    pub fn init(chunk_size: usize, max_retained_chunks: usize) ArenaAllocator {
        return .{
            .chunk_size = @max(chunk_size, 4096),
            .max_retained_chunks = max_retained_chunks,
        };
    }

    pub fn deinit(self: *ArenaAllocator) void {
        self.freeChain(self.current);
        self.freeChain(self.free_chunks);
        self.current = null;
        self.free_chunks = null;
        self.n_free_chunks = 0;
        self.committed_bytes = 0;
        self.total_allocated = 0;
    }

    fn freeChain(self: *ArenaAllocator, head: ?*Chunk) void {
        _ = self;
        var cur = head;
        while (cur) |ch| {
            const prev = ch.prev;
            std.heap.page_allocator.free(ch.ptr[0..ch.cap]);
            std.heap.c_allocator.destroy(ch);
            cur = prev;
        }
    }

    pub inline fn alloc(self: *ArenaAllocator, size: usize, align_bytes: usize) ?[*]u8 {
        const a = @max(align_bytes, @alignOf(u64));
        // Pages are 4096-aligned; anything larger cannot be honoured by chunking, so refuse
        // (callers turn this into a loud out-of-memory path) instead of returning an under-aligned pointer.
        if (a > 4096) return null;
        if (self.current) |ch| {
            const off = alignUp(ch.used, a);
            // overflow-safe: `off + size` could wrap for a huge `size`
            if (off <= ch.cap and size <= ch.cap - off) {
                const p = ch.ptr + off;
                ch.used = off + size;
                self.total_allocated += size;
                return p;
            }
        }
        return self.allocSlow(size, a);
    }

    pub inline fn allocZeroed(self: *ArenaAllocator, size: usize, align_bytes: usize) ?[*]u8 {
        const p = self.alloc(size, align_bytes) orelse return null;
        @memset(p[0..size], 0);
        return p;
    }

    noinline fn allocSlow(self: *ArenaAllocator, size: usize, align_bytes: usize) ?[*]u8 {
        // a request this large can never be satisfied (and `size + align` would overflow)
        if (size > std.math.maxInt(usize) / 2) return null;
        const need = @max(self.chunk_size, alignUp(size + align_bytes, 4096));
        var chosen: ?*Chunk = null;
        // Try recycling a cached chunk if it satisfies capacity.
        var prev_link: *?*Chunk = &self.free_chunks;
        while (prev_link.*) |cand| {
            if (cand.cap >= need) {
                prev_link.* = cand.prev;
                self.n_free_chunks -= 1;
                cand.used = 0;
                chosen = cand;
                break;
            }
            prev_link = &cand.prev;
        }
        if (chosen == null) {
            const mem = std.heap.page_allocator.alignedAlloc(u8, .fromByteUnits(4096), need) catch return null;
            const ch = std.heap.c_allocator.create(Chunk) catch {
                std.heap.page_allocator.free(mem);
                return null;
            };
            ch.* = .{ .ptr = mem.ptr, .cap = need, .used = 0, .prev = null };
            self.committed_bytes += need;
            chosen = ch;
        }
        const ch = chosen.?;
        ch.prev = self.current;
        self.current = ch;
        const off = alignUp(0, align_bytes);
        ch.used = off + size;
        self.total_allocated += size;
        return ch.ptr + off;
    }

    pub fn mark(self: *const ArenaAllocator) ArenaMark {
        return .{
            .chunk = self.current,
            .used = if (self.current) |c| c.used else 0,
            .total_bytes = self.total_allocated,
        };
    }

    pub fn restore(self: *ArenaAllocator, m: ArenaMark) void {
        while (self.current != m.chunk) {
            const ch = self.current orelse break;
            self.current = ch.prev;
            self.recycleOrReleaseChunk(ch);
        }
        if (self.current) |c| {
            if (c.used > m.used) poison(c.ptr[m.used..c.used]);
            c.used = m.used;
        }
        self.total_allocated = m.total_bytes;
    }

    /// Debug/ReleaseSafe builds overwrite rewound memory so a dangling reference to an object that
    /// escaped its scope reads 0xAA garbage (and fails loudly) instead of silently "working".
    inline fn poison(region: []u8) void {
        if (std.debug.runtime_safety) @memset(region, 0xAA);
    }

    /// Rewinds the entire arena in O(1), keeping one base chunk warm for the next frame/tick.
    /// An oversized base chunk (created for one big request) is released so it cannot pin memory forever.
    pub fn resetRetainingCapacity(self: *ArenaAllocator) void {
        var cur = self.current;
        if (cur == null) {
            self.total_allocated = 0;
            return;
        }
        while (cur.?.prev != null) {
            const top = cur.?;
            cur = top.prev;
            self.recycleOrReleaseChunk(top);
        }
        const base = cur.?;
        if (base.cap > self.chunk_size) {
            self.releaseChunk(base);
            self.current = null;
        } else {
            poison(base.ptr[0..base.used]);
            base.used = 0;
            self.current = base;
        }
        self.total_allocated = 0;
    }

    fn releaseChunk(self: *ArenaAllocator, ch: *Chunk) void {
        std.heap.page_allocator.free(ch.ptr[0..ch.cap]);
        self.committed_bytes -|= ch.cap;
        std.heap.c_allocator.destroy(ch);
    }

    fn recycleOrReleaseChunk(self: *ArenaAllocator, ch: *Chunk) void {
        poison(ch.ptr[0..ch.used]);
        ch.used = 0;
        if (ch.cap == self.chunk_size and self.n_free_chunks < self.max_retained_chunks) {
            ch.prev = self.free_chunks;
            self.free_chunks = ch;
            self.n_free_chunks += 1;
            return;
        }
        self.releaseChunk(ch);
    }

    /// Standard `std.mem.Allocator` view backed by this `ArenaAllocator` (zero per-alloc syscalls).
    pub fn allocator(self: *ArenaAllocator) std.mem.Allocator {
        return .{
            .ptr = self,
            .vtable = &arena_vtable,
        };
    }

    const arena_vtable = std.mem.Allocator.VTable{
        .alloc = vtAlloc,
        .resize = vtResize,
        .remap = vtRemap,
        .free = vtFree,
    };

    fn vtAlloc(ctx: *anyopaque, len: usize, alignment: std.mem.Alignment, _: usize) ?[*]u8 {
        const self: *ArenaAllocator = @ptrCast(@alignCast(ctx));
        return self.alloc(len, alignment.toByteUnits());
    }

    fn vtResize(ctx: *anyopaque, buf: []u8, _: std.mem.Alignment, new_len: usize, _: usize) bool {
        const self: *ArenaAllocator = @ptrCast(@alignCast(ctx));
        const ch = self.current orelse return false;
        const end_ptr = ch.ptr + ch.used;
        if (@intFromPtr(buf.ptr + buf.len) == @intFromPtr(end_ptr)) {
            const base_off = @intFromPtr(buf.ptr) - @intFromPtr(ch.ptr);
            if (base_off + new_len <= ch.cap) {
                ch.used = base_off + new_len;
                return true;
            }
        }
        return new_len <= buf.len;
    }

    fn vtRemap(ctx: *anyopaque, buf: []u8, alignment: std.mem.Alignment, new_len: usize, ra: usize) ?[*]u8 {
        if (vtResize(ctx, buf, alignment, new_len, ra)) return buf.ptr;
        const self: *ArenaAllocator = @ptrCast(@alignCast(ctx));
        const p = self.alloc(new_len, alignment.toByteUnits()) orelse return null;
        @memcpy(p[0..@min(buf.len, new_len)], buf[0..@min(buf.len, new_len)]);
        return p;
    }

    fn vtFree(ctx: *anyopaque, buf: []u8, _: std.mem.Alignment, _: usize) void {
        const self: *ArenaAllocator = @ptrCast(@alignCast(ctx));
        const ch = self.current orelse return;
        const end_ptr = ch.ptr + ch.used;
        if (@intFromPtr(buf.ptr + buf.len) == @intFromPtr(end_ptr)) {
            ch.used -= buf.len;
            self.total_allocated -|= buf.len;
        }
    }
};

/// Free node header chained inside free pool slots (matches first word of Java `Object`).
const FreeNode = extern struct {
    next: ?*FreeNode,
};

pub const SLAB_SIZE: usize = 128 * 1024;
pub const SLAB_HDR_BYTES: usize = 128; // preserves 16-byte/64-byte slot alignment inside every slab
const SLAB_MAGIC: u64 = 0x4A325A5F534C4142; // "J2Z_SLAB"
pub const MAX_RETAINED_EMPTY_SLABS: usize = 4; // 512 KiB max idle empty-slab reserve

/// Inline 128-byte header placed at byte 0 of every `SLAB_SIZE`-aligned slab block.
/// Enables O(1) `(ptr & ~(SLAB_SIZE - 1))` slab lookup and returning completely empty slabs to OS.
pub const SlabBlock = struct {
    magic: u64 = SLAB_MAGIC,
    data: [*]align(SLAB_SIZE) u8,
    free_head: ?*FreeNode = null,
    free_count: u32 = 0,
    nslots: u32 = 0,
    slot_size: u32 = 0,
    sc: u8 = 0,
    in_partial: bool = false,
    /// on SlabPool.unswept (linked through prev/next like the partial lists): marked, not yet swept
    in_unswept: bool = false,
    prev: ?*SlabBlock = null,
    next: ?*SlabBlock = null,
    all_prev: ?*SlabBlock = null,
    all_next: ?*SlabBlock = null,

    comptime {
        if (@sizeOf(SlabBlock) > SLAB_HDR_BYTES) @compileError("SlabBlock exceeds SLAB_HDR_BYTES");
    }
};

/// Zero-STW deterministic memory pool for persistent & semi-persistent objects and native subsystems.
/// Combines SLAB_SIZE-aligned segregated slabs + thread-local caches + 3-ring quiescent epoch
/// retirement + automatic empty-slab unmapping back to the OS.
pub const SlabPool = struct {
    pub const NUM_EPOCHS = 3;

    lock: std.Thread.Mutex = .{},
    epoch: std.atomic.Value(u64) = std.atomic.Value(u64).init(1),
    all_slabs_head: ?*SlabBlock = null,
    partial_slabs: [NUM_SC]?*SlabBlock = [_]?*SlabBlock{null} ** NUM_SC,
    empty_slabs: ?*SlabBlock = null,
    n_empty_slabs: usize = 0,
    committed_bytes: std.atomic.Value(usize) = std.atomic.Value(usize).init(0),
    live_bytes: std.atomic.Value(usize) = std.atomic.Value(usize).init(0),
    reclaimed_bytes: std.atomic.Value(usize) = std.atomic.Value(usize).init(0),

    /// base address of every mapped slab (the collector's conservative pointer test) and their address range
    slab_set: std.AutoHashMapUnmanaged(usize, void) = .empty,
    lo: usize = std.math.maxInt(usize),
    hi: usize = 0,

    // lazy sweeping (see startLazySweepLocked)
    unswept: [NUM_SC]?*SlabBlock = [_]?*SlabBlock{null} ** NUM_SC,
    n_unswept: usize = 0,
    gc_mark: u32 = 0,
    gc_alloc: u32 = 0,
    gc_mon_shift: u5 = 0,
    /// releases the monitors of dead objects (set by the runtime)
    release_monitors: ?*const fn ([]const u32) void = null,
    keep_empty_bytes: usize = 0,
    kept_empty: usize = 0,

    retired_lock: std.Thread.Mutex = .{},
    retired: [NUM_EPOCHS][NUM_SC]?*FreeNode = [_][NUM_SC]?*FreeNode{[_]?*FreeNode{null} ** NUM_SC} ** NUM_EPOCHS,
    retired_counts: [NUM_EPOCHS]usize = [_]usize{0} ** NUM_EPOCHS,

    inline fn slabOf(ptr: *anyopaque) ?*SlabBlock {
        const base = @intFromPtr(ptr) & ~(SLAB_SIZE - 1);
        if (base == 0) return null;
        const b: *SlabBlock = @ptrFromInt(base);
        if (b.magic != SLAB_MAGIC) return null;
        return b;
    }

    fn linkPartial(self: *SlabPool, b: *SlabBlock) void {
        if (b.in_partial) return;
        const sc: usize = b.sc;
        b.prev = null;
        b.next = self.partial_slabs[sc];
        if (self.partial_slabs[sc]) |h| h.prev = b;
        self.partial_slabs[sc] = b;
        b.in_partial = true;
    }

    fn unlinkPartial(self: *SlabPool, b: *SlabBlock) void {
        if (!b.in_partial) return;
        const sc: usize = b.sc;
        if (b.prev) |p| p.next = b.next else self.partial_slabs[sc] = b.next;
        if (b.next) |n| n.prev = b.prev;
        b.prev = null;
        b.next = null;
        b.in_partial = false;
    }

    fn linkAll(self: *SlabPool, b: *SlabBlock) void {
        b.all_prev = null;
        b.all_next = self.all_slabs_head;
        if (self.all_slabs_head) |h| h.all_prev = b;
        self.all_slabs_head = b;
    }

    fn unlinkAll(self: *SlabPool, b: *SlabBlock) void {
        if (b.all_prev) |p| p.all_next = b.all_next else self.all_slabs_head = b.all_next;
        if (b.all_next) |n| n.all_prev = b.all_prev;
        b.all_prev = null;
        b.all_next = null;
    }

    fn returnNodeLocked(self: *SlabPool, node: *FreeNode) void {
        const b = slabOf(node) orelse return;
        node.next = b.free_head;
        b.free_head = node;
        b.free_count += 1;

        if (b.free_count == b.nslots) {
            self.releaseEmptyLocked(b);
            return;
        }
        if (!b.in_partial) {
            self.linkPartial(b);
        }
    }

    /// `b` has no allocated slot left: keep it as a warm empty slab or unmap it.
    fn releaseEmptyLocked(self: *SlabPool, b: *SlabBlock) void {
        {
            self.unlinkPartial(b);
            b.free_head = null;
            b.free_count = 0;
            b.nslots = 0;
            b.slot_size = 0;
            if (self.n_empty_slabs < MAX_RETAINED_EMPTY_SLABS) {
                // Keep small warm reserve across size classes after dropping payload pages 4096..SLAB_SIZE from RSS.
                if (comptime @hasDecl(std.posix.MADV, "DONTNEED")) {
                    const tail_pages: [*]align(4096) u8 = @alignCast(b.data + 4096);
                    std.posix.madvise(tail_pages, SLAB_SIZE - 4096, std.posix.MADV.DONTNEED) catch {};
                }
                b.next = self.empty_slabs;
                self.empty_slabs = b;
                self.n_empty_slabs += 1;
            } else {
                // Unmap excess completely empty slabs straight back to the OS!
                self.unlinkAll(b);
                _ = self.slab_set.remove(@intFromPtr(b.data));
                b.magic = 0;
                const mem = b.data[0..SLAB_SIZE];
                std.heap.page_allocator.free(mem);
                _ = self.committed_bytes.fetchSub(SLAB_SIZE, .monotonic);
            }
        }
    }

    /// Allocates a batch of chained slots for size class `sc` from existing partial slabs or a fresh slab.
    pub fn refillBatch(self: *SlabPool, sc: usize, max_batch: usize) ?*anyopaque {
        return self.refillBatchEx(sc, max_batch, true);
    }

    /// refillBatch that maps no new slab when `may_grow` is false (heap ceiling reached).
    pub fn refillBatchEx(self: *SlabPool, sc: usize, max_batch: usize, may_grow: bool) ?*anyopaque {
        self.lock.lock();
        defer self.lock.unlock();

        var head_out: ?*FreeNode = null;
        var taken: usize = 0;
        while (taken < max_batch) {
            if (self.partial_slabs[sc] == null and !self.sweepFor(sc)) break;
            const b = self.partial_slabs[sc].?;
            while (taken < max_batch) {
                const node = b.free_head orelse break;
                b.free_head = node.next;
                b.free_count -= 1;
                node.next = head_out;
                head_out = node;
                taken += 1;
            }
            if (b.free_count == 0) {
                self.unlinkPartial(b);
            }
        }
        if (head_out != null) return @ptrCast(head_out);

        // Carve a `SLAB_SIZE`-aligned slab block (either warm empty slab or fresh mmap).
        const b: *SlabBlock = if (self.empty_slabs) |eb| blk: {
            self.empty_slabs = eb.next;
            eb.next = null;
            self.n_empty_slabs -= 1;
            // old slots' header words would look like live objects to a collector. madvise dropped the pages
            // after the first only where the OS page size is 4 KiB, so clear the whole payload.
            @memset(eb.data[SLAB_HDR_BYTES..SLAB_SIZE], 0);
            break :blk eb;
        } else if (self.stealEmptyLocked()) |eb| blk: {
            // swept empty: its old slots' header words are 0 but the new size class puts headers elsewhere
            @memset(eb.data[SLAB_HDR_BYTES..SLAB_SIZE], 0);
            break :blk eb;
        } else blk: {
            if (!may_grow) return null;
            const mem = std.heap.page_allocator.alignedAlloc(u8, .fromByteUnits(SLAB_SIZE), SLAB_SIZE) catch return null;
            self.slab_set.put(std.heap.c_allocator, @intFromPtr(mem.ptr), {}) catch {
                std.heap.page_allocator.free(mem);
                return null;
            };
            // Allocator.alignedAlloc fills new memory with 0xAA in safe build modes: every free slot must start
            // with a zero header (a collector reads gc_bits of free slots)
            @memset(mem, 0);
            self.lo = @min(self.lo, @intFromPtr(mem.ptr));
            self.hi = @max(self.hi, @intFromPtr(mem.ptr) + SLAB_SIZE);
            const hdr: *SlabBlock = @ptrCast(mem.ptr);
            hdr.* = .{ .data = mem.ptr };
            self.linkAll(hdr);
            _ = self.committed_bytes.fetchAdd(SLAB_SIZE, .monotonic);
            break :blk hdr;
        };

        const slot = SIZE_CLASSES[sc];
        const nslots: u32 = @intCast((SLAB_SIZE - SLAB_HDR_BYTES) / slot);
        b.slot_size = slot;
        b.nslots = nslots;
        b.sc = @intCast(sc);
        b.in_partial = false;
        b.free_head = null;
        b.free_count = 0;

        const give: u32 = @min(nslots, @as(u32, @intCast(max_batch)));
        var head_give: ?*FreeNode = null;
        var i: u32 = give;
        while (i > 0) {
            i -= 1;
            const node: *FreeNode = @ptrCast(@alignCast(b.data + SLAB_HDR_BYTES + @as(usize, i) * slot));
            node.next = head_give;
            head_give = node;
        }
        var j: u32 = nslots;
        while (j > give) {
            j -= 1;
            const node: *FreeNode = @ptrCast(@alignCast(b.data + SLAB_HDR_BYTES + @as(usize, j) * slot));
            node.next = b.free_head;
            b.free_head = node;
            b.free_count += 1;
        }
        if (b.free_count > 0) {
            self.linkPartial(b);
        }
        return @ptrCast(head_give);
    }

    /// The slab whose base address is `base`, if one is mapped there. Caller holds `lock` (or the world is stopped).
    pub fn slabAt(self: *const SlabPool, base: usize) ?*SlabBlock {
        if (!self.slab_set.contains(base)) return null;
        return @ptrFromInt(base);
    }

    /// Lowest and highest address any slab ever occupied.
    pub fn addressRange(self: *const SlabPool) struct { lo: usize, hi: usize } {
        return .{ .lo = self.lo, .hi = self.hi };
    }

    /// After a collection has marked the heap (world stopped, caller holds `lock`, thread caches emptied, no
    /// slab unswept): every formatted slab goes on its size class's unswept list. An allocated slot has
    /// `alloc_bit` set in the u32 at byte 12 (the object header's gc_bits) and survives if `mark` is set too.
    /// Slabs are swept one at a time when an allocation of their size class needs room (or another class needs
    /// an empty slab), and the rest by finishSweepLocked before the next collection marks: the pause holds only
    /// the marking. Up to `keep_empty_bytes` of slabs found empty stay with their size class (the allocation
    /// volume until the next collection: unmapping and refaulting them each cycle costs more than collecting).
    pub fn startLazySweepLocked(self: *SlabPool, mark: u32, alloc_bit: u32, mon_shift: u5, keep_empty_bytes: usize) void {
        self.gc_mark = mark;
        self.gc_alloc = alloc_bit;
        self.gc_mon_shift = mon_shift;
        self.keep_empty_bytes = keep_empty_bytes;
        self.kept_empty = 0;
        for (&self.partial_slabs) |*head| {
            var x = head.*;
            while (x) |b| {
                x = b.next;
                b.prev = null;
                b.next = null;
                b.in_partial = false;
            }
            head.* = null;
        }
        var cur = self.all_slabs_head;
        while (cur) |b| : (cur = b.all_next) {
            if (b.nslots == 0) continue; // warm empty slab (chained on empty_slabs through next)
            std.debug.assert(!b.in_unswept);
            const sc: usize = b.sc;
            b.prev = null;
            b.next = self.unswept[sc];
            if (self.unswept[sc]) |h| h.prev = b;
            self.unswept[sc] = b;
            b.in_unswept = true;
            self.n_unswept += 1;
        }
    }

    fn popUnswept(self: *SlabPool, sc: usize) ?*SlabBlock {
        const b = self.unswept[sc] orelse return null;
        self.unswept[sc] = b.next;
        if (b.next) |n| n.prev = null;
        b.next = null;
        b.prev = null;
        b.in_unswept = false;
        self.n_unswept -= 1;
        return b;
    }

    const SweepResult = enum { full, partial, empty_kept, released };

    /// Sweep one slab taken off the unswept lists: dead slots become free with gc_bits 0, marks are cleared,
    /// the monitors of dead objects are released. Touches only `b` (which nobody else can reach while it is
    /// off every list), so it may run without `lock`; linkSwept files the slab afterwards (under `lock`).
    fn sweepSlots(self: *SlabPool, b: *SlabBlock) void {
        var dead: [64]u32 = undefined;
        var n_dead: usize = 0;
        var free_head: ?*FreeNode = null;
        var nfree: u32 = 0;
        var i: u32 = b.nslots;
        while (i > 0) {
            i -= 1;
            const slot = b.data + SLAB_HDR_BYTES + @as(usize, i) * b.slot_size;
            const bits: *u32 = @ptrCast(@alignCast(slot + 12));
            const v = bits.*;
            if (v & self.gc_alloc != 0) {
                if (v & self.gc_mark != 0) {
                    bits.* = v & ~self.gc_mark;
                    continue;
                }
                const mi = v >> self.gc_mon_shift;
                if (mi != 0) {
                    if (n_dead == dead.len) {
                        self.release_monitors.?(dead[0..n_dead]);
                        n_dead = 0;
                    }
                    dead[n_dead] = mi;
                    n_dead += 1;
                }
                bits.* = 0;
            }
            const node: *FreeNode = @ptrCast(@alignCast(slot));
            node.next = free_head;
            free_head = node;
            nfree += 1;
        }
        if (n_dead > 0) self.release_monitors.?(dead[0..n_dead]);
        b.free_head = free_head;
        b.free_count = nfree;
    }

    fn linkSwept(self: *SlabPool, b: *SlabBlock) SweepResult {
        const nfree = b.free_count;
        if (nfree == b.nslots) {
            if (self.kept_empty + SLAB_SIZE <= self.keep_empty_bytes) {
                self.kept_empty += SLAB_SIZE;
                self.linkPartial(b);
                return .empty_kept;
            }
            self.releaseEmptyLocked(b);
            return .released;
        }
        if (nfree > 0) {
            self.linkPartial(b);
            return .partial;
        }
        return .full;
    }

    /// Sweep unswept slabs of `sc` until one has a free slot. False when none is left.
    /// Called and returns with `lock` held, but sweeps with it released (other threads keep allocating).
    fn sweepFor(self: *SlabPool, sc: usize) bool {
        while (self.popUnswept(sc)) |b| {
            self.lock.unlock();
            self.sweepSlots(b);
            self.lock.lock();
            _ = self.linkSwept(b);
            if (self.partial_slabs[sc] != null) return true;
        }
        return false;
    }

    fn sweepSlabLocked(self: *SlabPool, b: *SlabBlock) SweepResult {
        self.sweepSlots(b);
        return self.linkSwept(b);
    }

    /// An empty slab from another size class's unswept slabs (sweeping them), for reformatting.
    fn stealEmptyLocked(self: *SlabPool) ?*SlabBlock {
        for (0..NUM_SC) |sc| {
            while (self.popUnswept(sc)) |b| {
                switch (self.sweepSlabLocked(b)) {
                    .empty_kept => {
                        self.unlinkPartial(b);
                        self.kept_empty -= SLAB_SIZE;
                        return b;
                    },
                    .released => if (self.empty_slabs) |eb| {
                        self.empty_slabs = eb.next;
                        eb.next = null;
                        self.n_empty_slabs -= 1;
                        return eb;
                    },
                    else => {},
                }
            }
        }
        return null;
    }

    /// Sweep every slab still unswept (caller holds `lock`): before a collection marks.
    pub fn finishSweepLocked(self: *SlabPool) void {
        for (0..NUM_SC) |sc| {
            while (self.popUnswept(sc)) |b| _ = self.sweepSlabLocked(b);
        }
    }

    /// Immediately returns a fixed-size slot back to its owning slab (and OS if slab becomes empty).
    pub fn returnSlotImmediate(self: *SlabPool, ptr: *anyopaque, size: usize) void {
        const sc = sizeClass(size);
        const node: *FreeNode = @ptrCast(@alignCast(ptr));
        self.lock.lock();
        self.returnNodeLocked(node);
        self.lock.unlock();
        _ = self.reclaimed_bytes.fetchAdd(SIZE_CLASSES[sc], .monotonic);
        _ = self.live_bytes.fetchSub(@min(SIZE_CLASSES[sc], self.live_bytes.load(.monotonic)), .monotonic);
    }

    /// Retires an object/buffer into the current quiescent epoch bin so concurrent readers cannot observe UAF.
    pub fn retireDeferred(self: *SlabPool, ptr: *anyopaque, size: usize) void {
        const sc = sizeClass(size);
        const ep = self.epoch.load(.acquire);
        const bucket = ep % NUM_EPOCHS;
        const node: *FreeNode = @ptrCast(@alignCast(ptr));
        self.retired_lock.lock();
        node.next = self.retired[bucket][sc];
        self.retired[bucket][sc] = node;
        self.retired_counts[bucket] += 1;
        self.retired_lock.unlock();
    }

    /// Advances epoch and recycles slots from `epoch - 2` back into owning slabs (returning empty slabs to OS).
    /// Returns number of bytes recycled.
    pub fn advanceAndReclaim(self: *SlabPool) usize {
        const new_ep = self.epoch.fetchAdd(1, .acq_rel) + 1;
        const reclaim_bucket = (new_ep + 1) % NUM_EPOCHS; // safe 2 epochs behind active writers/readers

        self.retired_lock.lock();
        const cnt = self.retired_counts[reclaim_bucket];
        if (cnt == 0) {
            self.retired_lock.unlock();
            return 0;
        }
        const local_lists = self.retired[reclaim_bucket];
        self.retired[reclaim_bucket] = [_]?*FreeNode{null} ** NUM_SC;
        self.retired_counts[reclaim_bucket] = 0;
        self.retired_lock.unlock();

        var reclaimed: usize = 0;
        self.lock.lock();
        for (0..NUM_SC) |sc| {
            var cur_node = local_lists[sc];
            while (cur_node) |n| {
                const nxt = n.next;
                self.returnNodeLocked(n);
                reclaimed += SIZE_CLASSES[sc];
                cur_node = nxt;
            }
        }
        self.lock.unlock();

        if (reclaimed > 0) {
            _ = self.reclaimed_bytes.fetchAdd(reclaimed, .monotonic);
        }
        return reclaimed;
    }

    pub fn deinit(self: *SlabPool) void {
        self.lock.lock();
        defer self.lock.unlock();
        var cur = self.all_slabs_head;
        while (cur) |b| {
            const nxt = b.all_next;
            b.magic = 0;
            std.heap.page_allocator.free(b.data[0..SLAB_SIZE]);
            cur = nxt;
        }
        self.all_slabs_head = null;
        self.partial_slabs = [_]?*SlabBlock{null} ** NUM_SC;
        self.empty_slabs = null;
        self.n_empty_slabs = 0;
        self.committed_bytes.store(0, .monotonic);
    }
};

test "ArenaAllocator mark/restore/reset handles large & small bursts without leak" {
    var arena = ArenaAllocator.init(64 * 1024, 4);
    defer arena.deinit();

    const m0 = arena.mark();
    var p1 = arena.alloc(128, 16) orelse return error.OutOfMemory;
    p1[0] = 0x5A;
    _ = arena.alloc(70 * 1024, 32) orelse return error.OutOfMemory; // triggers chunk growth
    arena.restore(m0);
    try std.testing.expectEqual(@as(usize, 0), arena.total_allocated);

    // std.mem.Allocator compatibility
    const alloc = arena.allocator();
    var list = std.ArrayListUnmanaged(u32){};
    for (0..1024) |i| try list.append(alloc, @intCast(i));
    try std.testing.expectEqual(@as(u32, 1023), list.items[1023]);

    arena.resetRetainingCapacity();
    try std.testing.expectEqual(@as(usize, 0), arena.total_allocated);
    try std.testing.expect(arena.current != null);
}

test "SlabPool refills batches and reclaims retired slots across epochs without STW" {
    var pool = SlabPool{};
    defer pool.deinit();

    const raw = pool.refillBatch(sizeClass(64), 32) orelse return error.OutOfMemory;
    const node: *FreeNode = @ptrCast(@alignCast(raw));
    pool.retireDeferred(node, 64);

    // Must advance through NUM_EPOCHS - 1 -> bucket wraps safely after 2 advances.
    try std.testing.expectEqual(@as(usize, 0), pool.advanceAndReclaim());
    const reclaimed = pool.advanceAndReclaim();
    try std.testing.expectEqual(@as(usize, 64), reclaimed);
    try std.testing.expect(pool.partial_slabs[sizeClass(64)] != null);
}

test "SlabPool returns completely empty slabs across epochs back to the OS" {
    var pool = SlabPool{};
    defer pool.deinit();

    // Allocate enough 16 KiB batches to force > 8 slabs (> 1 MiB committed), then return them all.
    var slots: [16]?*anyopaque = [_]?*anyopaque{null} ** 16;
    const sc16k = sizeClass(16384);
    var count: usize = 0;
    while (count < 16) {
        var cur: ?*FreeNode = @ptrCast(@alignCast(pool.refillBatch(sc16k, 4) orelse return error.OutOfMemory));
        while (cur) |n| {
            const nxt = n.next;
            if (count < 16) {
                slots[count] = n;
                count += 1;
            } else {
                pool.returnSlotImmediate(n, 16384);
            }
            cur = nxt;
        }
    }
    const peak_committed = pool.committed_bytes.load(.monotonic);
    try std.testing.expect(peak_committed >= 2 * SLAB_SIZE);

    for (slots) |s| {
        if (s) |p| pool.retireDeferred(p, 16384);
    }
    _ = pool.advanceAndReclaim();
    _ = pool.advanceAndReclaim();
    _ = pool.advanceAndReclaim();
    // Every slab is now empty: excess empty slabs above MAX_RETAINED_EMPTY_SLABS were unmapped
    // and partial_slabs[sc16k] is clean.
    try std.testing.expect(pool.partial_slabs[sc16k] == null);
    try std.testing.expect(pool.committed_bytes.load(.monotonic) <= MAX_RETAINED_EMPTY_SLABS * SLAB_SIZE);
}

test "ArenaAllocator rejects overflowing requests instead of wrapping" {
    var arena = ArenaAllocator.init(64 * 1024, 2);
    defer arena.deinit();
    _ = arena.alloc(128, 16) orelse return error.OutOfMemory;
    try std.testing.expect(arena.alloc(std.math.maxInt(usize) - 8, 16) == null);
    try std.testing.expect(arena.alloc(std.math.maxInt(usize), 16) == null);
    // arena is still usable afterwards
    try std.testing.expect(arena.alloc(64, 16) != null);
}

test "ArenaAllocator poisons rewound memory and drops oversized base chunk on reset" {
    var arena = ArenaAllocator.init(64 * 1024, 2);
    defer arena.deinit();

    const m = arena.mark();
    const p = arena.alloc(256, 16) orelse return error.OutOfMemory;
    @memset(p[0..256], 0x11);
    arena.restore(m);
    if (std.debug.runtime_safety) {
        // the escaped pointer now reads poison, not the old object bytes
        try std.testing.expectEqual(@as(u8, 0xAA), p[0]);
        try std.testing.expectEqual(@as(u8, 0xAA), p[255]);
    }

    // one 1 MiB request creates an oversized chunk; a reset must not keep pinning it
    arena.resetRetainingCapacity();
    _ = arena.alloc(1024 * 1024, 16) orelse return error.OutOfMemory;
    try std.testing.expect(arena.committed_bytes >= 1024 * 1024);
    arena.resetRetainingCapacity();
    try std.testing.expect(arena.committed_bytes < 1024 * 1024);
    try std.testing.expectEqual(@as(usize, 0), arena.total_allocated);
    try std.testing.expect(arena.alloc(64, 16) != null);
}

test "ArenaAllocator free accounting stays consistent for LIFO frees" {
    var arena = ArenaAllocator.init(64 * 1024, 2);
    defer arena.deinit();
    const a = arena.allocator();
    const buf = try a.alloc(u8, 1000);
    try std.testing.expectEqual(@as(usize, 1000), arena.total_allocated);
    a.free(buf);
    try std.testing.expectEqual(@as(usize, 0), arena.total_allocated);
}
