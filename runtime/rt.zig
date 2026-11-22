//! monolith runtime: object model, mark-sweep GC over a SlabPool heap, exceptions,
//! monitors, class initialization (JVMS 5.5), threads and safepoints.
//!
//! Memory
//! ------
//! * Small objects (`<= 32 KiB`) come from thread-local size-class caches refilled from the segregated
//!   `SlabPool` (`mem_pool.zig`); large ones from malloc with a hidden list header (`LargeHdr`).
//! * Objects the transpiler proves method-local, and objects allocated inside `pushArenaScope`, bump in the
//!   thread's scratch `ArenaAllocator` (header bit ARENA) and are rewound, never collected.
//! * Non-moving stop-the-world mark & sweep with parallel marking (see "collector"): stacks and registers
//!   are scanned conservatively, the heap precisely. MONO_HEAP_MAX_MB caps the heap (OutOfMemoryError
//!   beyond it), MONO_GC_MIN_MB / MONO_GC_FACTOR tune the trigger, MONO_GC_VERBOSE=1 logs every collection.
const std = @import("std");
const shardmode = @import("shardmode.zig");
const builtin = @import("builtin");
pub const mem_pool = @import("mem_pool.zig");
pub const thread_pool = @import("thread_pool.zig");

/// All mutable process-wide runtime state lives in `Shared` structs. In a single-unit build the storage
/// is a plain local; in a sharded build (shardmode.zig) it is an extern symbol defined once, in the
/// generated shared.o, so that every separately compiled shard sees the same object.
pub fn Shared(comptime T: type, comptime name: []const u8, comptime reserve: usize) type {
    return struct {
        var bytes: [@sizeOf(T)]u8 align(64) = [_]u8{0} ** @sizeOf(T);
        pub const ptr: *T = if (shardmode.sharded) blk: {
            if (@sizeOf(T) > reserve) @compileError("shared state " ++ name ++ " exceeds its reserved size");
            const p = @extern(*align(64) u8, .{ .name = "mono_shared_" ++ name });
            break :blk @ptrCast(@alignCast(p));
        } else @ptrCast(&bytes);
    };
}
/// Upper bound of compile units (shards) of a sharded build; monolith/shard.py refuses more.
pub const MAX_SHARDS = 256;
pub const StateT = struct {
    patch_verify: bool = false,
    // one per shard (each shard's runtime copy has its own thread-locals to set): see MAX_SHARDS
    cur_setters: [MAX_SHARDS]?*const fn (*ThreadState) void = [_]?*const fn (*ThreadState) void{null} ** MAX_SHARDS,
    n_setters: usize = 0,
    ipv6_cached: i32 = -1,
    pre_close_fd: c_int = -1,
    start_ms: i64 = 0,
    hooks: Hooks = undefined,
    threads_lock: std.Thread.Mutex = .{},
    threads_head: ?*ThreadState = null,
    orphans: ?*Object = null,
    thread_count: std.atomic.Value(u32) = std.atomic.Value(u32).init(0),
    gc_count: u64 = 0,
    live_bytes: usize = 0,
    gc_requested: std.atomic.Value(bool) = std.atomic.Value(bool).init(false),
    gc_lock: std.Thread.Mutex = .{},
    gc_done_mutex: std.Thread.Mutex = .{},
    gc_done_cond: std.Thread.Condition = .{},
    bytes_since_gc: std.atomic.Value(usize) = std.atomic.Value(usize).init(0),
    /// smallest collection trigger (bytes allocated since the last GC); MONO_GC_MIN_MB overrides
    gc_min_threshold: usize = 256 * 1024 * 1024,
    gc_threshold: usize = 256 * 1024 * 1024,
    /// next collection when bytes allocated since the last one exceed live * gc_factor_pct / 100 (MONO_GC_FACTOR)
    gc_factor_pct: usize = 100,
    /// heap ceiling in bytes (MONO_HEAP_MAX_MB, 0: none): allocation beyond it collects, then throws OutOfMemoryError
    gc_max_heap: usize = 0,
    /// bytes held by large (malloc'd) objects
    large_bytes: std.atomic.Value(usize) = std.atomic.Value(usize).init(0),
    mark_stack: std.ArrayList(*Object) = .empty,
    refs_seen: std.ArrayList(*Object) = .empty,
    pending_refs: std.ArrayList(Ref) = .empty,
    pending_lock: std.Thread.Mutex = .{},
    /// large objects sorted by address, rebuilt at every collection (conservative pointer lookup)
    large_index: std.ArrayList(*Object) = .empty,
    /// MONO_ALLOC_PROFILE=1: heap allocation samples (one per PROF_STEP bytes per thread) by class and allocating
    /// method (innermost frame), printed with every collection
    alloc_profile: bool = false,
    prof_lock: std.Thread.Mutex = .{},
    prof: std.AutoHashMapUnmanaged(ProfKey, u64) = .empty,
    extra_roots: std.ArrayList(*Ref) = .empty,
    extra_roots_lock: std.Thread.Mutex = .{},
    dyn_classes: std.ArrayList(*Class) = .empty,
    dyn_classes_lock: std.Thread.Mutex = .{},
    gc_verbose: bool = false,
    gc_total_ns: u64 = 0,
    gc_max_pause_ns: u64 = 0,
    /// System.gc() is a hint: a request this soon (ms) after a collection ended is skipped, so a caller that asks
    /// over and over (direct-buffer reservation, engine housekeeping) cannot keep the game stopped (MONO_GC_EXPLICIT_GAP_MS)
    gc_explicit_gap_ms: i64 = 10_000,
    /// most garbage (bytes allocated since the last collection) the heap may collect before the next one, whatever the
    /// live size: keeps the heap from growing past what the phone can spare (MONO_GC_MAX_GARBAGE_MB, 0 = no cap)
    gc_max_garbage: usize = 0,
    /// MONO_HEAP_CENSUS=N: print live bytes by class at every Nth collection (0 = off)
    census_every: u32 = 0,
    /// start of initRuntime, for the startup timeline lines (MONO_GC_VERBOSE)
    t0_ns: i128 = 0,
    gc_last_end_ms: i64 = 0,
    gc_explicit_skipped: u64 = 0,
    gc_caller: ?*const MethodMeta = null,
    /// last collection: time to stop the threads, to mark, to sweep
    gc_sync_ns: u64 = 0,
    gc_mark_ns: u64 = 0,
    gc_sweep_ns: u64 = 0,
    /// part of the last pause spent sweeping slabs the allocator had not swept since the collection before
    gc_finish_ns: u64 = 0,
    jni_globals: std.ArrayList(Ref) = .empty,
    jni_globals_lock: std.Thread.Mutex = .{},
    heap_blocks_bytes: usize = 0,
    slab_pool: mem_pool.SlabPool = .{},
    workers: thread_pool.WorkerPool = .{},
    workers_lock: std.Thread.Mutex = .{},
    workers_started: std.atomic.Value(bool) = std.atomic.Value(bool).init(false),
    frame_epoch: std.atomic.Value(u64) = std.atomic.Value(u64).init(1),
    /// monitor side table: index -> *Monitor in chunks of MON_CHUNK entries (see monitorOf)
    mon_lock: std.Thread.Mutex = .{},
    mon_dir: ?[*]?[*]?*Monitor = null,
    mon_next: u32 = 1,
    mon_free: std.ArrayList(u32) = .empty,
    oom_error: Ref = null,
    init_lock: std.Thread.Mutex = .{},
    init_cond: std.Thread.Condition = .{},
    trace_init_cached: i8 = -1,
    prim_Z: @TypeOf(primClass("boolean", 'Z', 1)) = primClass("boolean", 'Z', 1),
    prim_B: @TypeOf(primClass("byte", 'B', 1)) = primClass("byte", 'B', 1),
    prim_C: @TypeOf(primClass("char", 'C', 2)) = primClass("char", 'C', 2),
    prim_S: @TypeOf(primClass("short", 'S', 2)) = primClass("short", 'S', 2),
    prim_I: @TypeOf(primClass("int", 'I', 4)) = primClass("int", 'I', 4),
    prim_J: @TypeOf(primClass("long", 'J', 8)) = primClass("long", 'J', 8),
    prim_F: @TypeOf(primClass("float", 'F', 4)) = primClass("float", 'F', 4),
    prim_D: @TypeOf(primClass("double", 'D', 8)) = primClass("double", 'D', 8),
    prim_V: @TypeOf(primClass("void", 'V', 0)) = primClass("void", 'V', 0),
    array_class_lock: std.Thread.Mutex = .{},
    array_ifaces: [2]*Class = undefined,
    hash_seed: @TypeOf(std.atomic.Value(u32).init(0x9E3779B9)) = std.atomic.Value(u32).init(0x9E3779B9),
    intern_map: std.HashMapUnmanaged([]const u16, Ref, U16Ctx, 80) = .empty,
    intern_lock: std.Thread.Mutex = .{},
    mirror_lock: std.Thread.Mutex = .{},
    argv_storage: [][*:0]u8 = &.{},
    user_props: [][*:0]u8 = &.{},
    main_ts: ThreadState = .{},
    exit_code: u8 = 0,
};
pub const st: *StateT = Shared(StateT, "rt", 1 << 20).ptr;
pub const hooks = &st.hooks;
const threads_lock = &st.threads_lock;
const gc_requested = &st.gc_requested;
const gc_lock = &st.gc_lock;
const gc_done_mutex = &st.gc_done_mutex;
const gc_done_cond = &st.gc_done_cond;
const bytes_since_gc = &st.bytes_since_gc;
/// Why the running collection was requested (verbose log): 1 allocation volume, 2 no free slab, 3 large object past the
/// ceiling, 4 large allocation failed, 5 System.gc(), 6 internal (shutdown)
var gc_why: u8 = 0;
const GC_WHY_NAMES = [_][]const u8{ "?", "volume", "no free slab", "large past ceiling", "large alloc failed", "System.gc", "internal" };
const mark_stack = &st.mark_stack;
const refs_seen = &st.refs_seen;
const pending_refs = &st.pending_refs;
const pending_lock = &st.pending_lock;
pub const thread_count = &st.thread_count;
const extra_roots = &st.extra_roots;
const extra_roots_lock = &st.extra_roots_lock;
const dyn_classes = &st.dyn_classes;
const dyn_classes_lock = &st.dyn_classes_lock;
pub const jni_globals = &st.jni_globals;
pub const jni_globals_lock = &st.jni_globals_lock;
const init_lock = &st.init_lock;
const init_cond = &st.init_cond;
pub const prim_Z = &st.prim_Z;
pub const prim_B = &st.prim_B;
pub const prim_C = &st.prim_C;
pub const prim_S = &st.prim_S;
pub const prim_I = &st.prim_I;
pub const prim_J = &st.prim_J;
pub const prim_F = &st.prim_F;
pub const prim_D = &st.prim_D;
pub const prim_V = &st.prim_V;
const array_class_lock = &st.array_class_lock;
const array_ifaces = &st.array_ifaces;
const hash_seed = &st.hash_seed;
const intern_map = &st.intern_map;
const intern_lock = &st.intern_lock;
const mirror_lock = &st.mirror_lock;
const main_ts = &st.main_ts;
pub const E = error{Throw};

/// Return convention of value-returning Java methods (the generated <name>_r functions):
/// value and error flag packed into one or two machine words, returned in registers.
pub fn Res(comptime T: type) type {
    return switch (T) {
        i32, f32 => packed struct(u64) { v: u32, e: u32 },
        i64, f64, Ref => packed struct(u128) { v: u64, e: u64 },
        else => @compileError("no register return convention for " ++ @typeName(T)),
    };
}

pub inline fn toR(comptime T: type, x: E!T) Res(T) {
    const v = x catch return .{ .v = 0, .e = 1 };
    return .{ .v = switch (T) {
        Ref => @intFromPtr(v),
        else => @bitCast(v),
    }, .e = 0 };
}

pub inline fn unR(comptime T: type, r: Res(T)) E!T {
    if (r.e != 0) return error.Throw;
    return switch (T) {
        Ref => @ptrFromInt(r.v),
        else => @bitCast(r.v),
    };
}
pub const Ref = ?*Object;

// ------------------------------------------------------------------ object model
pub const Object = extern struct {
    class: *Class,
    hash: u32,
    /// bit 0 MARK, bit 1 ALLOC, bit 2 ARENA (thread scratch arena, not collected), bits 3..31 monitor index
    /// (0: no monitor), see monitorOf
    gc_bits: u32,
};

comptime {
    if (@sizeOf(Object) != 16) @compileError("object header must stay 16 bytes");
}

pub const Array = extern struct {
    hdr: Object,
    length: i32,
    _pad: u32,
};

pub const Kind = enum(u8) { instance, interface, array, primitive };

pub const FieldMeta = struct {
    name: []const u8,
    offset: u32,
    flags: u32 = 0,
    desc: []const u8 = "",
    signature: ?[]const u8 = null,
    annotations: ?[]const u8 = null,
};

pub const StaticFieldMeta = struct {
    name: []const u8,
    ptr: *anyopaque,
    flags: u32 = 0,
    desc: []const u8 = "",
    signature: ?[]const u8 = null,
    annotations: ?[]const u8 = null,
};

/// Compact constant pool entry (only what annotation bytes reference; see monolith/reflectmeta.py).
pub const CpEntry = union(enum) {
    none,
    utf8: []const u8,
    int: i32,
    long: i64,
    float: u32,
    double: u64,
};

/// Class-level reflection metadata (generic signature, annotations, nesting).
pub const ClassReflect = struct {
    signature: ?[]const u8 = null,
    annotations: ?[]const u8 = null,
    cpool: []const CpEntry = &.{},
    /// InnerClasses entry for this class
    is_inner: bool = false,
    outer: ?[]const u8 = null,
    inner_name: ?[]const u8 = null,
    inner_flags: u32 = 0,
    /// EnclosingMethod
    enclosing_class: ?[]const u8 = null,
    enclosing_name: ?[]const u8 = null,
    enclosing_desc: ?[]const u8 = null,
    /// member classes declared by this class (InnerClasses entries whose outer is this class)
    declared_classes: []const []const u8 = &.{},
    nest_host: ?[]const u8 = null,
};

pub const ServiceEntry = struct {
    service: []const u8,
    create: *const fn () E!Ref,
};

/// Declared method metadata (reflection: getDeclaredMethods/Constructors).
pub const MethodInfoMeta = struct {
    name: []const u8,
    desc: []const u8,
    flags: u32,
    /// compiled function (null if RTA pruned it or it is abstract) and its invoker shape
    fp: ?*const anyopaque = null,
    shape: u32 = 0,
    signature: ?[]const u8 = null,
    annotations: ?[]const u8 = null,
    param_annotations: ?[]const u8 = null,
    annotation_default: ?[]const u8 = null,
    exceptions: []const []const u8 = &.{},
};

pub const IEntry = struct {
    iface: *Class,
    methods: []const *const anyopaque,
};

pub const Class = struct {
    name: []const u8,
    kind: Kind = .instance,
    flags: u32 = 0,
    has_default: bool = false,
    super: ?*Class = null,
    interfaces: []const *Class = &.{},
    size: u32 = @sizeOf(Object),
    ref_offsets: []const u32 = &.{},
    vtable: []const *const anyopaque = &.{},
    itable: []const IEntry = &.{},
    clinit: ?*const fn () E!void = null,
    static_refs: []const *Ref = &.{},
    elem: ?*Class = null,
    elem_size: u32 = 0,
    prim: u8 = 0,
    enum_values: ?*const fn () Res(Ref) = null,
    fields: []const FieldMeta = &.{},
    static_fields: []const StaticFieldMeta = &.{},
    methods: []const MethodInfoMeta = &.{},
    refl: ?*const ClassReflect = null,
    /// defined by the boot loader (JDK) rather than the application class loader
    boot: bool = false,
    /// subclass of java.lang.ref.Reference: its referent is traced weakly
    is_ref: bool = false,
    // mutable runtime state
    init_state: u8 = 0,
    init_thread: ?*ThreadState = null,
    mirror: Ref = null,
    array_class: ?*Class = null,
    id: u32 = 0,
    init_error: Ref = null,
    init_error_thread: Ref = null,
};

pub const INIT_NONE: u8 = 0;
pub const INIT_RUNNING: u8 = 1;
pub const INIT_DONE: u8 = 2;
pub const INIT_ERROR: u8 = 3;

pub const MethodMeta = struct {
    /// declaring class (null for runtime-internal frames)
    cls: ?*Class = null,
    class: []const u8,
    name: []const u8,
    desc: []const u8,
    file: []const u8 = "",
};

pub const Frame = struct {
    prev: ?*Frame = null,
    refs: ?[*]Ref = null,
    n: u32 = 0,
    meth: *const MethodMeta,
    /// current source line (updated before instructions that can throw), 0 = unknown
    line: u32 = 0,
};

// ------------------------------------------------------------------ hooks (set by generated code)
pub const Exc = enum(u8) {
    NPE,
    Arithmetic,
    ArrayIndex,
    ClassCast,
    NegativeArraySize,
    ArrayStore,
    OutOfMemory,
    IllegalMonitorState,
    UnsatisfiedLink,
    NoClassDefFound,
    AbstractMethod,
    Interrupted,
    CloneNotSupported,
    StackOverflow,
    UnsupportedOperation,
    IncompatibleClassChange,
    NoSuchField,
    NoSuchMethod,
    IllegalArgument,
    InternalError,
    IllegalThreadState,
    Instantiation,
    IO,
    FileNotFound,
    OutOfMemoryNative,
};

pub const Hooks = struct {
    make_exception: *const fn (Exc, ?[]const u8) E!Ref,
    wrap_init_error: *const fn (Ref) E!Ref,
    is_error: *const fn (Ref) bool,
    make_string: *const fn ([]const u16) E!Ref,
    string_chars: *const fn (*Object) []u16,
    object_class: *Class,
    class_class: *Class,
    string_class: *Class,
    cloneable: *Class,
    serializable: *Class,
    all_classes: []const *Class,
    mirror_set_class: *const fn (*Object, *Class) void,
    mirror_get_class: *const fn (*Object) *Class,
    thread_run: *const fn (Ref) E!void,
    thread_uncaught: *const fn (Ref, Ref) E!void,
    thread_start_main: *const fn () E!Ref,
    shutdown: *const fn () E!void,
    thread_eetop_set: *const fn (*Object, i64) void,
    referent_offset: u32,
    ref_queue_offset: u32,
    enqueue_ref: *const fn (Ref) E!void,
    make_ncdfe: *const fn (*Class) E!Ref,
    new_throwable: *const fn (*Class, ?[]const u8) E!Ref,
    foreign_thread: *const fn () E!Ref,
};

// ------------------------------------------------------------------ threads
pub const RUNNING: u8 = 0;
pub const SAFE: u8 = 1;

pub const ThreadState = struct {
    top: ?*Frame = null,
    exc: Ref = null,
    java_thread: Ref = null,
    state: std.atomic.Value(u8) = std.atomic.Value(u8).init(SAFE),
    objects: ?*Object = null,
    next: ?*ThreadState = null,
    prev: ?*ThreadState = null,
    stack_limit: usize = 0,
    /// highest address of this thread's stack (conservative scan bound)
    stack_hi: usize = 0,
    /// callee-saved registers and stack pointer captured when the thread last left RUNNING (spillRegs);
    /// the collector scans [saved_regs[SP_SLOT], stack_hi) and these words conservatively
    saved_regs: [SAVED_REGS]usize = [_]usize{0} ** SAVED_REGS,
    /// snapshots taken when Java called into native code (enterNative), innermost last: a callback returning
    /// into that native code restores the matching one (see enterSafeFromCallback)
    jni_snaps: [MAX_NATIVE_DEPTH][SAVED_REGS]usize = undefined,
    jni_depth: u32 = 0,
    /// what method entry compares the frame address with: stack_limit, or maxInt while a collection is
    /// requested (the entry safepoint poll rides on the stack check, see pushFrame)
    poll_limit: usize = 0,
    handles: [8]Ref = [_]Ref{null} ** 8,
    interrupted: std.atomic.Value(bool) = std.atomic.Value(bool).init(false),
    park_mutex: std.Thread.Mutex = .{},
    park_cond: std.Thread.Condition = .{},
    /// 0 = empty, 1 = permit available, 2 = thread waiting in park_cond
    park_state: std.atomic.Value(u32) = std.atomic.Value(u32).init(0),
    /// per-size-class free lists of the small-object slab pool
    cache: [NUM_SC]?*Object = [_]?*Object{null} ** NUM_SC,
    /// per-thread bump arena for transient/scoped zero-malloc frame allocations (max 2 chunks = 512 KiB warm)
    scratch_arena: mem_pool.ArenaAllocator = mem_pool.ArenaAllocator.init(mem_pool.DEFAULT_CHUNK_SIZE, 2),
    use_arena_depth: u32 = 0,
    /// heap bytes allocated since this thread's last allocation-profile sample (MONO_ALLOC_PROFILE)
    prof_acc: usize = 0,
    /// heap bytes allocated since the last flush into st.bytes_since_gc (flushed every 64 KiB)
    alloc_acc: usize = 0,
    sleeper: ?*Monitor = null,
    alive: bool = true,
    /// JNI: this thread's JNIEnv (a pointer to a function table; libraries such as LWJGL may
    /// replace it with their own copy of the table) and its local references.
    jni_env: ?*const anyopaque = null,
    jni_locals: std.ArrayList(Ref) = .empty,
};

pub threadlocal var cur: *ThreadState = undefined;
threadlocal var processing_refs: bool = false;

/// libc malloc: its state is shared by all program shards (smp_allocator keeps per-shard global state)
pub const gpa = std.heap.c_allocator;

fn registerThread(t: *ThreadState) void {
    lockSafe(threads_lock);
    defer threads_lock.unlock();
    t.next = st.threads_head;
    t.prev = null;
    if (st.threads_head) |h| h.prev = t;
    st.threads_head = t;
}

fn unregisterThread(t: *ThreadState) void {
    t.scratch_arena.deinit();
    lockSafe(threads_lock);
    defer threads_lock.unlock();
    if (t.prev) |p| p.next = t.next else st.threads_head = t.next;
    if (t.next) |n| n.prev = t.prev;
    // return unused cached slots directly back to SlabPool with zero GC needed
    for (&t.cache, 0..) |*slot, sc| {
        var cur_o = slot.*;
        while (cur_o) |obj| {
            const nxt = slotNext(obj).*;
            st.slab_pool.returnSlotImmediate(obj, mem_pool.SIZE_CLASSES[sc]);
            cur_o = nxt;
        }
        slot.* = null;
    }
    // hand surviving large objects to the orphan list
    var o = t.objects;
    while (o) |obj| {
        const nx = largeNext(obj).*;
        largeNext(obj).* = st.orphans;
        st.orphans = obj;
        o = nx;
    }
    t.objects = null;
}

// ------------------------------------------------------------------ safepoints
// Stop-the-world with cooperative safepoints: generated code polls `safepoint()` at loop back-edges and
// method entry (pushFrameT compares against poll_limit, which a collector raises to maxInt); threads that
// block or run native code switch to SAFE so the collector does not wait for them.

pub const SAVED_REGS: usize = 16;
/// index of the saved stack pointer in ThreadState.saved_regs
const SP_SLOT: usize = SAVED_REGS - 1;

/// Store the callee-saved registers and the stack pointer into t.saved_regs. Must stay a real call: the
/// registers it reads are the caller's (it uses none of them itself), and every caller-saved register that
/// is live across it has been spilled into a frame above the stack pointer it records.
noinline fn spillRegs(t: *ThreadState) void {
    const p: [*]usize = &t.saved_regs;
    switch (builtin.cpu.arch) {
        .aarch64 => asm volatile (
            \\stp x19, x20, [%[p], #0]
            \\stp x21, x22, [%[p], #16]
            \\stp x23, x24, [%[p], #32]
            \\stp x25, x26, [%[p], #48]
            \\stp x27, x28, [%[p], #64]
            \\str x29, [%[p], #80]
            \\mov x9, sp
            \\str x9, [%[p], #120]
            :
            : [p] "r" (p),
            : .{ .x9 = true, .memory = true }),
        .x86_64 => asm volatile (
            \\movq %%rbx, 0(%%rdi)
            \\movq %%rbp, 8(%%rdi)
            \\movq %%r12, 16(%%rdi)
            \\movq %%r13, 24(%%rdi)
            \\movq %%r14, 32(%%rdi)
            \\movq %%r15, 40(%%rdi)
            \\movq %%rsp, 120(%%rdi)
            :
            : [p] "{rdi}" (p),
            : .{ .memory = true }),
        else => @compileError("monolith GC: spillRegs has no register list for this architecture"),
    }
}

pub inline fn safepoint() void {
    if (gc_requested.load(.acquire)) parkForGC();
}

noinline fn parkForGC() void {
    enterSafe();
    leaveSafe();
}

/// Leave RUNNING (blocking call or native code that does not touch the Java heap). Inline on purpose: the
/// registers spillRegs captures must be the ones of the function that blocks.
pub inline fn enterSafe() void {
    const t = cur;
    spillRegs(t);
    t.state.store(SAFE, .seq_cst);
}

pub const MAX_NATIVE_DEPTH: usize = 16;

/// Java code calls foreign code (JNI, JNA, libffi) that may call back into Java: SAFE for the duration, with
/// the snapshot kept for enterSafeFromCallback. Pair with leaveNative in the same function.
pub inline fn enterNative() void {
    const t = cur;
    spillRegs(t);
    if (t.jni_depth >= MAX_NATIVE_DEPTH) @panic("monolith: Java -> native -> Java nesting deeper than rt.MAX_NATIVE_DEPTH");
    t.jni_snaps[t.jni_depth] = t.saved_regs;
    t.jni_depth += 1;
    t.state.store(SAFE, .seq_cst);
}

pub inline fn leaveNative() void {
    leaveSafe();
    cur.jni_depth -= 1;
}

/// A JNI function or libffi upcall returns into foreign code. The Java frames under that code are what they
/// were when it was entered, so the snapshot taken then is restored: a fresh one, taken in a frame that is
/// about to be popped, could miss caller registers saved only in that frame. A thread with no Java frames
/// under the foreign code (created natively) takes a fresh one.
pub inline fn enterSafeFromCallback() void {
    const t = cur;
    if (t.jni_depth > 0) t.saved_regs = t.jni_snaps[t.jni_depth - 1] else spillRegs(t);
    t.state.store(SAFE, .seq_cst);
}

/// Back to RUNNING; waits while a collection is in progress. Inline, like enterSafe: the frame that captured
/// the registers must stay live (no tail call can pop it) until the thread runs again, and while it waits it
/// leaves its snapshot alone (the collector may be reading it).
pub inline fn leaveSafe() void {
    const t = cur;
    while (true) {
        t.state.store(RUNNING, .seq_cst);
        if (!gc_requested.load(.seq_cst)) return;
        t.state.store(SAFE, .seq_cst);
        gc_done_mutex.lock();
        while (gc_requested.load(.seq_cst)) gc_done_cond.wait(gc_done_mutex);
        gc_done_mutex.unlock();
    }
}

/// Lock a mutex that other mutators may hold for a long time without blocking a collection.
pub inline fn lockSafe(m: *std.Thread.Mutex) void {
    if (m.tryLock()) return;
    enterSafe();
    m.lock();
    leaveSafe();
}

// ------------------------------------------------------------------ allocation & GC
/// Smallest collection trigger (bytes allocated since the last GC). Large enough that start-up, which
/// allocates a few GiB of mostly long-lived data, does not collect every 64 MiB; MONO_GC_MIN_MB overrides.

/// JNI global references (each NewGlobalRef appends; DeleteGlobalRef removes one occurrence).

pub fn addRoot(slot: *Ref) void {
    extra_roots_lock.lock();
    defer extra_roots_lock.unlock();
    extra_roots.append(gpa, slot) catch @panic("out of memory");
}

pub fn objectSize(o: *Object) usize {
    const c = o.class;
    if (c.kind == .array) {
        const a: *Array = @ptrCast(@alignCast(o));
        return @sizeOf(Array) + @as(usize, @intCast(a.length)) * c.elem_size;
    }
    return c.size;
}

/// Hidden prefix in front of every large object (allocated with malloc): links it into the
/// owning thread's large-object list (`ThreadState.objects`, `st.orphans`).
const LargeHdr = extern struct {
    next: ?*Object,
    _pad: usize,
};

inline fn largeNext(o: *Object) *?*Object {
    const h: *LargeHdr = @ptrFromInt(@intFromPtr(o) - @sizeOf(LargeHdr));
    return &h.next;
}

fn rawAlloc(size: usize) ?*Object {
    const mem = gpa.alignedAlloc(u8, .@"16", size + @sizeOf(LargeHdr)) catch return null;
    _ = st.large_bytes.fetchAdd(size + @sizeOf(LargeHdr), .monotonic);
    @memset(mem, 0);
    return @ptrCast(@alignCast(mem.ptr + @sizeOf(LargeHdr)));
}

/// Free slots of a block are chained through their first word (where a live object has its class).
inline fn slotNext(o: *Object) *?*Object {
    return @ptrCast(o);
}

// ---------------------------------------------------------------- small objects (SlabPool) and arena
pub const NUM_SC = mem_pool.NUM_SC;
const MAX_SMALL: usize = mem_pool.MAX_SMALL;
const MARK: u32 = 1;
const ALLOC: u32 = 2;
const ARENA: u32 = 4;
const MON_SHIFT: u5 = 3;

inline fn sizeClass(size: usize) usize {
    return mem_pool.sizeClass(size);
}

/// Bytes in use: live after the last collection plus allocated since.
pub fn usedBytes() usize {
    return st.live_bytes + bytes_since_gc.load(.monotonic);
}

/// Bytes the Java heap holds from the OS: slabs plus large objects.
pub fn heapBytes() usize {
    return st.slab_pool.committed_bytes.load(.monotonic) + st.large_bytes.load(.monotonic);
}

fn refill(sc: usize) ?*Object {
    const slot_sz = mem_pool.SIZE_CLASSES[sc];
    // a whole slab's worth per refill: the pool lock is shared by every allocating thread (32-slot batches
    // made 4 allocating threads spend most of their time in futex waits). Collections empty the caches.
    const batch: usize = @max(2, mem_pool.SLAB_SIZE / slot_sz);
    // a new slab may not take the heap past its ceiling: the caller collects and retries first
    const may_grow = st.gc_max_heap == 0 or heapBytes() + mem_pool.SLAB_SIZE <= st.gc_max_heap;
    const head = st.slab_pool.refillBatchEx(sc, batch, may_grow) orelse return null;
    st.heap_blocks_bytes = heapBytes();
    return @ptrCast(@alignCast(head));
}

fn allocSmall(size: usize) ?*Object {
    const sc = sizeClass(size);
    const t = cur;
    var o = t.cache[sc];
    if (o == null) {
        o = refill(sc);
        if (o == null) return null;
    }
    const obj = o.?;
    t.cache[sc] = slotNext(obj).*;
    const p: [*]u8 = @ptrCast(obj);
    const end = @max(size, @sizeOf(Object));
    if (end <= 256) {
        var i: usize = @sizeOf(Object);
        const n = (end + 7) & ~@as(usize, 7);
        while (i < n) : (i += 8) @as(*u64, @ptrCast(@alignCast(p + i))).* = 0;
    } else {
        @memset(p[@sizeOf(Object)..end], 0);
    }
    obj.hash = 0;
    obj.gc_bits = ALLOC;
    return obj;
}

const ProfKey = struct { c: *Class, m: ?*const MethodMeta };
const PROF_STEP: usize = 256 * 1024;

noinline fn profSample(t: *ThreadState, c: *Class) void {
    const n = t.prof_acc / PROF_STEP;
    t.prof_acc %= PROF_STEP;
    const key = ProfKey{ .c = c, .m = if (t.top) |f| f.meth else null };
    st.prof_lock.lock();
    defer st.prof_lock.unlock();
    const e = st.prof.getOrPut(gpa, key) catch return;
    if (!e.found_existing) e.value_ptr.* = 0;
    e.value_ptr.* += n;
}

/// Top allocation sites since the previous report (world stopped, called by the collector), then reset.
fn profReport() void {
    const Ent = struct { k: ProfKey, v: u64 };
    var list: std.ArrayList(Ent) = .empty;
    defer list.deinit(gpa);
    var total: u64 = 0;
    var it = st.prof.iterator();
    while (it.next()) |e| {
        total += e.value_ptr.*;
        list.append(gpa, .{ .k = e.key_ptr.*, .v = e.value_ptr.* }) catch return;
    }
    std.mem.sort(Ent, list.items, {}, struct {
        fn gt(_: void, a: Ent, b: Ent) bool {
            return a.v > b.v;
        }
    }.gt);
    var buf: [512]u8 = undefined;
    writeFd(2, std.fmt.bufPrint(&buf, "[alloc profile: {d} MiB sampled since the previous collection]\n", .{total * PROF_STEP >> 20}) catch "");
    for (list.items[0..@min(15, list.items.len)]) |e| {
        const m = e.k.m;
        writeFd(2, std.fmt.bufPrint(&buf, "  {d:>6} MiB {d:>3}%  {s}  <-  {s}.{s}\n", .{
            e.v * PROF_STEP >> 20, if (total > 0) e.v * 100 / total else 0, e.k.c.name,
            if (m) |mm| mm.class else "?", if (m) |mm| mm.name else "?",
        }) catch "");
    }
    st.prof.clearRetainingCapacity();
}

pub fn allocObject(c: *Class, size: usize) E!*Object {
    const t0 = cur;
    if (t0.use_arena_depth > 0) return allocArenaObject(c, size);
    if (st.alloc_profile) {
        t0.prof_acc += size;
        if (t0.prof_acc >= PROF_STEP) profSample(t0, c);
    }
    // heap allocation volume drives collections (flushed in 64 KiB batches: one atomic per batch)
    t0.alloc_acc += size;
    if (t0.alloc_acc >= 64 * 1024) {
        const add = t0.alloc_acc;
        t0.alloc_acc = 0;
        if (bytes_since_gc.fetchAdd(add, .monotonic) + add > st.gc_threshold) {
            gc_why = 1;
            collect(false);
            if (pending_refs.items.len > 0) processPendingRefs();
        }
    }
    if (size <= MAX_SMALL) {
        var o = allocSmall(size);
        if (o == null) {
            gc_why = 2;
            collect(true);
            o = allocSmall(size);
            if (o == null) return throwOOM();
        }
        o.?.class = c;
        return o.?;
    }
    // the ceiling is about memory in use: empty slabs the pool keeps for reuse count in heapBytes() but cost no
    // allocation, and a heap pinned at its ceiling by garbage must not collect for every large object
    if (st.gc_max_heap != 0 and (usedBytes() + size > st.gc_max_heap or heapBytes() + size > st.gc_max_heap + st.gc_max_heap / 4)) {
        gc_why = 3;
        collect(true);
        if (usedBytes() + size > st.gc_max_heap) return throwOOM();
    }
    var mem = rawAlloc(size);
    if (mem == null) {
        gc_why = 4;
        collect(true);
        mem = rawAlloc(size);
        if (mem == null) return throwOOM();
    }
    const o = mem.?;
    o.class = c;
    o.gc_bits = ALLOC;
    largeNext(o).* = t0.objects;
    t0.objects = o;
    return o;
}

fn throwOOM() E {
    // Preallocated in startup so OOM can always be thrown.
    cur.exc = st.oom_error;
    return error.Throw;
}

/// System.gc(): a full collection, then enqueue the references it cleared.
pub fn gc() void {
    const now = std.time.milliTimestamp();
    if (st.gc_explicit_gap_ms > 0 and st.gc_last_end_ms != 0 and now - st.gc_last_end_ms < st.gc_explicit_gap_ms) {
        st.gc_explicit_skipped += 1;
        return;
    }
    gc_why = 5;
    st.gc_caller = if (cur.top) |f| f.meth else null;
    collect(true);
    if (pending_refs.items.len > 0) processPendingRefs();
}

/// A collection nobody may skip (the renderer was torn down).
pub fn gcNow() void {
    gc_why = 6;
    collect(true);
    if (pending_refs.items.len > 0) processPendingRefs();
}

// ---------------------------------------------------------------- collector
// Non-moving mark & sweep, stop-the-world.
// Roots: every thread's stack and saved registers (scanned conservatively: generated code keeps references
// in ordinary locals), the used part of every thread's scratch arena (objects there are never collected, but
// their fields are roots), precise shadow-stack refs where a frame has them, thread exception / Thread object
// / handles / JNI locals, class statics and mirrors, interned strings, JNI globals and extra roots.
// The heap itself is traced precisely (Class.ref_offsets, reference arrays).

fn collect(force: bool) void {
    enterSafe();
    gc_lock.lock();
    // We hold gc_lock: no other collection can start, so becoming RUNNING again cannot park.
    cur.state.store(RUNNING, .seq_cst);
    defer gc_lock.unlock();
    if (!force and bytes_since_gc.load(.monotonic) <= st.gc_threshold) return;
    const gc_t0 = std.time.nanoTimestamp();
    threads_lock.lock();
    defer threads_lock.unlock();
    gc_requested.store(true, .seq_cst);
    var t = st.threads_head;
    while (t) |th| : (t = th.next) @atomicStore(usize, &th.poll_limit, std.math.maxInt(usize), .monotonic);
    // wait for every other mutator to reach a safe state
    t = st.threads_head;
    while (t) |th| : (t = th.next) {
        if (th == cur) continue;
        var spins: u64 = 0;
        var warned = false;
        while (th.state.load(.seq_cst) == RUNNING) {
            spins += 1;
            if (spins < 1000) {
                std.Thread.yield() catch {};
            } else {
                std.Thread.sleep(100 * std.time.ns_per_us);
                if (!warned and spins > 1000 + 20_000) { // about 2 s
                    warned = true;
                    reportStuckThread(th);
                }
            }
        }
    }
    const pause_t0 = std.time.nanoTimestamp();
    st.gc_sync_ns = @intCast(pause_t0 - gc_t0);
    spillRegs(cur);
    markAndSweep();
    gc_done_mutex.lock();
    gc_requested.store(false, .seq_cst);
    t = st.threads_head;
    while (t) |th| : (t = th.next) @atomicStore(usize, &th.poll_limit, th.stack_limit, .monotonic);
    gc_done_cond.broadcast();
    gc_done_mutex.unlock();
    const now = std.time.nanoTimestamp();
    st.gc_total_ns += @intCast(now - gc_t0);
    const pause: u64 = @intCast(now - pause_t0);
    if (pause > st.gc_max_pause_ns) st.gc_max_pause_ns = pause;
}

/// A thread kept running for seconds while a collection waits: it is in native code or a blocking call that
/// did not switch to SAFE. Say which, loudly (the wait goes on: stopping it any other way is not safe).
noinline fn reportStuckThread(th: *ThreadState) void {
    var buf: [512]u8 = undefined;
    const m = if (th.top) |f| f.meth else &rt_meta;
    const msg = std.fmt.bufPrint(&buf, "monolith GC: waiting for thread {*} to reach a safepoint for 2 s; its innermost Java frame is {s}.{s}{s} (a blocking or native call there lacks enterSafe/leaveSafe)\n", .{ th, m.class, m.name, m.desc }) catch "monolith GC: a thread does not reach a safepoint\n";
    writeFd(2, msg);
}

/// Enqueue references cleared by the last collection (like HotSpot's Reference Handler,
/// but run by the thread that triggered the GC, outside of the collector).
fn processPendingRefs() void {
    if (processing_refs) return;
    processing_refs = true;
    defer processing_refs = false;
    while (true) {
        lockSafe(pending_lock);
        const r = if (pending_refs.items.len > 0) pending_refs.pop() else null;
        pending_lock.unlock();
        const ref = r orelse return;
        var fr_refs = [_]Ref{ref};
        var fr = Frame{ .refs = &fr_refs, .n = 1, .meth = &rt_meta };
        pushFrame(&fr) catch return;
        hooks.enqueue_ref(ref) catch {
            cur.exc = null;
        };
        popFrame(&fr);
    }
}

inline fn markRef(r: Ref) void {
    markRefTo(r, mark_stack);
}

/// Mark `r` and queue it on `stack` (safe to call from several GC workers at once: the mark bit is set
/// with an atomic OR and only the thread that flips it queues the object). Arena objects are skipped:
/// they are never freed and their memory is scanned as a root.
inline fn markRefTo(r: Ref, stack: *std.ArrayList(*Object)) void {
    if (r) |o| {
        const bits = @atomicLoad(u32, &o.gc_bits, .monotonic);
        if (bits & (MARK | ALLOC | ARENA) != ALLOC) return;
        const old = @atomicRmw(u32, &o.gc_bits, .Or, MARK, .monotonic);
        if (old & MARK == 0) stack.append(gpa, o) catch @panic("GC mark stack: out of memory");
    }
}

fn markClass(c: *Class) void {
    markRef(c.mirror);
    markRef(c.init_error);
    markRef(c.init_error_thread);
    for (c.static_refs) |p| markRef(p.*);
}

/// Address range the heap occupies (slabs and large objects), computed at the start of a collection:
/// a word outside it is not a pointer into the heap.
var heap_lo: usize = 0;
var heap_hi: usize = 0;

/// The live object a word points into (start or interior), or null. World stopped.
fn findObject(w: usize) ?*Object {
    if (w < heap_lo or w >= heap_hi) return null;
    const base = w & ~(mem_pool.SLAB_SIZE - 1);
    if (st.slab_pool.slabAt(base)) |b| {
        if (b.slot_size == 0) return null;
        const off = w - base;
        if (off < mem_pool.SLAB_HDR_BYTES) return null;
        const idx = (off - mem_pool.SLAB_HDR_BYTES) / b.slot_size;
        if (idx >= b.nslots) return null;
        const o: *Object = @ptrFromInt(base + mem_pool.SLAB_HDR_BYTES + idx * b.slot_size);
        if (o.gc_bits & ALLOC == 0) return null;
        return o;
    }
    // large objects: last one starting at or below w
    const items = st.large_index.items;
    var lo: usize = 0;
    var hi: usize = items.len;
    while (lo < hi) {
        const mid = (lo + hi) / 2;
        if (@intFromPtr(items[mid]) <= w) lo = mid + 1 else hi = mid;
    }
    if (lo == 0) return null;
    const o = items[lo - 1];
    if (w < @intFromPtr(o) + objectSize(o)) return o;
    return null;
}

fn scanConservative(lo: usize, hi: usize) void {
    var p = std.mem.alignForward(usize, lo, @sizeOf(usize));
    while (p + @sizeOf(usize) <= hi) : (p += @sizeOf(usize)) {
        const w = @as(*const usize, @ptrFromInt(p)).*;
        if (findObject(w)) |o| markRef(o);
    }
}

fn buildHeapIndex() void {
    const li = &st.large_index;
    li.clearRetainingCapacity();
    var t = st.threads_head;
    while (t) |th| : (t = th.next) {
        var o = th.objects;
        while (o) |obj| : (o = largeNext(obj).*) li.append(gpa, obj) catch @panic("GC: out of memory");
    }
    var o = st.orphans;
    while (o) |obj| : (o = largeNext(obj).*) li.append(gpa, obj) catch @panic("GC: out of memory");
    std.mem.sort(*Object, li.items, {}, struct {
        fn lt(_: void, a: *Object, b: *Object) bool {
            return @intFromPtr(a) < @intFromPtr(b);
        }
    }.lt);
    heap_lo = std.math.maxInt(usize);
    heap_hi = 0;
    if (li.items.len > 0) {
        heap_lo = @intFromPtr(li.items[0]);
        const last = li.items[li.items.len - 1];
        heap_hi = @intFromPtr(last) + objectSize(last);
        // an earlier, longer object can end past the last one's end
        for (li.items) |x| heap_hi = @max(heap_hi, @intFromPtr(x) + objectSize(x));
    }
    const r = st.slab_pool.addressRange();
    if (r.hi > r.lo) {
        heap_lo = @min(heap_lo, r.lo);
        heap_hi = @max(heap_hi, r.hi);
    }
}

const GcPool = struct {
    lock: std.Thread.Mutex = .{},
    pool: std.ArrayList(*Object) = .empty,
    idle: std.atomic.Value(u32) = std.atomic.Value(u32).init(0),
    n: u32 = 1,
    refs_lock: std.Thread.Mutex = .{},
    go: std.atomic.Value(bool) = std.atomic.Value(bool).init(false),
    /// bytes of the small objects marked
    live: std.atomic.Value(usize) = std.atomic.Value(usize).init(0),
};

fn traceObject(o: *Object, local: *std.ArrayList(*Object), pool: *GcPool, live: *usize) void {
    const c = o.class;
    const size = objectSize(o);
    if (size <= MAX_SMALL) live.* += size; // large objects are counted by their sweep
    if (c.kind == .array) {
        if (c.elem_size == @sizeOf(Ref) and c.elem.?.kind != .primitive) {
            const a: *Array = @ptrCast(@alignCast(o));
            const data = arrayData(Ref, a);
            for (data[0..@intCast(a.length)]) |r| markRefTo(r, local);
        }
    } else {
        if (c.is_ref) {
            pool.refs_lock.lock();
            refs_seen.append(gpa, o) catch @panic("GC: out of memory");
            pool.refs_lock.unlock();
        }
        const base: [*]u8 = @ptrCast(o);
        for (c.ref_offsets) |off| {
            const p: *Ref = @ptrCast(@alignCast(base + off));
            markRefTo(p.*, local);
        }
    }
}

const SPILL: usize = 4096;
const CHUNK: usize = 1024;

fn traceWorker(pool: *GcPool) void {
    while (!pool.go.load(.acquire)) std.Thread.yield() catch {};
    var local: std.ArrayList(*Object) = .empty;
    defer local.deinit(gpa);
    var live: usize = 0;
    defer _ = pool.live.fetchAdd(live, .monotonic);
    while (true) {
        while (local.pop()) |o| {
            traceObject(o, &local, pool, &live);
            if (local.items.len > SPILL) {
                pool.lock.lock();
                const keep = local.items.len - SPILL / 2;
                pool.pool.appendSlice(gpa, local.items[keep..]) catch @panic("GC: out of memory");
                pool.lock.unlock();
                local.shrinkRetainingCapacity(keep);
            }
        }
        // out of local work: take a chunk from the shared pool, or wait for the others to finish
        var idle_counted = false;
        var spins: u32 = 0;
        while (true) {
            pool.lock.lock();
            const have = pool.pool.items.len;
            if (have > 0) {
                const take = @min(have, CHUNK);
                local.appendSlice(gpa, pool.pool.items[have - take ..]) catch @panic("GC: out of memory");
                pool.pool.shrinkRetainingCapacity(have - take);
                pool.lock.unlock();
                if (idle_counted) _ = pool.idle.fetchSub(1, .acq_rel);
                break;
            }
            pool.lock.unlock();
            if (!idle_counted) {
                _ = pool.idle.fetchAdd(1, .acq_rel);
                idle_counted = true;
            } else if (pool.idle.load(.acquire) == pool.n) {
                // everybody is idle and the pool is empty: nothing can add work any more
                pool.lock.lock();
                const empty = pool.pool.items.len == 0;
                pool.lock.unlock();
                if (empty) return;
            }
            spins += 1;
            if (spins < 64) {
                std.Thread.yield() catch {};
            } else {
                std.Thread.sleep(50 * std.time.ns_per_us);
            }
        }
    }
}

const MAX_GC_WORKERS: usize = 8;

/// Marking threads: every core but one (the world is stopped, so nothing else needs them), at most 8.
fn gcWorkers() u32 {
    const cpus = std.Thread.getCpuCount() catch 1;
    return @intCast(@max(1, @min(cpus -| 1, MAX_GC_WORKERS)));
}

/// Trace from the mark stack; returns the bytes of the small objects marked.
fn traceParallel() usize {
    var pool = GcPool{};
    pool.n = gcWorkers();
    if (mark_stack.items.len < 64) pool.n = 1; // nothing worth sharing
    pool.pool.appendSlice(gpa, mark_stack.items) catch @panic("GC: out of memory");
    mark_stack.clearRetainingCapacity();
    var threads: [MAX_GC_WORKERS - 1]?std.Thread = [_]?std.Thread{null} ** (MAX_GC_WORKERS - 1);
    var started: u32 = 1;
    while (started < pool.n) : (started += 1) {
        threads[started - 1] = std.Thread.spawn(.{}, traceWorker, .{&pool}) catch break;
    }
    pool.n = started; // workers wait for `go`, so they all see the final count
    pool.go.store(true, .release);
    traceWorker(&pool);
    for (&threads) |*t| if (t.*) |th| th.join();
    pool.pool.deinit(gpa);
    return pool.live.load(.monotonic);
}

fn markThreadRoots(th: *ThreadState) void {
    markRef(th.exc);
    markRef(th.java_thread);
    for (th.handles) |h| markRef(h);
    for (th.jni_locals.items) |h| markRef(h);
    var f = th.top;
    while (f) |fr| : (f = fr.prev) {
        if (fr.refs) |refs| for (refs[0..fr.n]) |r| markRef(r);
    }
    const sp = th.saved_regs[SP_SLOT];
    if (sp != 0 and th.stack_hi > sp) {
        for (th.saved_regs[0..SP_SLOT]) |w| if (findObject(w)) |o| markRef(o);
        scanConservative(sp, th.stack_hi);
    }
    var ch = th.scratch_arena.current;
    while (ch) |c| : (ch = c.prev) scanConservative(@intFromPtr(c.ptr), @intFromPtr(c.ptr) + c.used);
}

fn markAndSweep() void {
    st.gc_count += 1;
    st.slab_pool.lock.lock();
    defer st.slab_pool.lock.unlock();
    // thread caches hold free slots: they go back to their slabs in the sweep
    var t = st.threads_head;
    while (t) |th| : (t = th.next) for (&th.cache) |*c| {
        c.* = null;
    };
    const mark_t0 = std.time.nanoTimestamp();
    // slabs left unswept since the last collection still carry its marks: sweep them first
    st.slab_pool.finishSweepLocked();
    const finish_ns = std.time.nanoTimestamp();
    buildHeapIndex();
    // roots
    t = st.threads_head;
    while (t) |th| : (t = th.next) markThreadRoots(th);
    for (hooks.all_classes) |c| markClass(c);
    for (dyn_classes.items) |c| markClass(c);
    for (prim_classes) |c| markClass(c);
    for (extra_roots.items) |p| markRef(p.*);
    for (jni_globals.items) |g| markRef(g);
    markRef(st.oom_error);
    for (pending_refs.items) |r| markRef(r);
    {
        var it = intern_map.valueIterator();
        while (it.next()) |v| markRef(v.*);
    }
    const roots_ns = std.time.nanoTimestamp();
    // trace (parallel: the root set above is the initial work pool)
    var live: usize = traceParallel();
    // weak/soft/phantom references: clear referents that were not marked
    for (refs_seen.items) |r| {
        const base: [*]u8 = @ptrCast(r);
        const p: *Ref = @ptrCast(@alignCast(base + hooks.referent_offset));
        if (p.*) |referent| {
            if (referent.gc_bits & (MARK | ALLOC | ARENA) == ALLOC) {
                p.* = null;
                const q: *Ref = @ptrCast(@alignCast(base + hooks.ref_queue_offset));
                if (q.* != null) pending_refs.append(gpa, r) catch {};
            }
        }
    }
    refs_seen.clearRetainingCapacity();
    if (st.census_every != 0 and st.gc_count % st.census_every == 0) heapCensus();
    const sweep_t0 = std.time.nanoTimestamp();
    st.gc_mark_ns = @intCast(sweep_t0 - mark_t0);
    // sweep: large objects now, slabs lazily as allocation needs them (startLazySweepLocked)
    t = st.threads_head;
    while (t) |th| : (t = th.next) th.objects = sweepList(th.objects, &live);
    st.orphans = sweepList(st.orphans, &live);
    const garbage_cap: usize = if (st.gc_max_garbage != 0) st.gc_max_garbage else std.math.maxInt(usize);
    var next = @max(st.gc_min_threshold, @min(live / 100 * st.gc_factor_pct, garbage_cap));
    if (st.gc_max_heap != 0) {
        // collect again before the heap reaches its ceiling (at least 16 MiB of headroom to allocate in)
        const room = if (st.gc_max_heap > live) st.gc_max_heap - live else 0;
        next = @max(16 * 1024 * 1024, @min(next, room - room / 8));
    }
    st.slab_pool.release_monitors = &releaseMonitors;
    st.slab_pool.startLazySweepLocked(MARK, ALLOC, MON_SHIFT, next);
    st.heap_blocks_bytes = heapBytes();
    st.live_bytes = live;
    st.gc_sweep_ns = @intCast(std.time.nanoTimestamp() - sweep_t0);
    st.gc_finish_ns = @intCast(finish_ns - mark_t0);
    st.gc_threshold = next;
    bytes_since_gc.store(0, .monotonic);
    st.gc_last_end_ms = std.time.milliTimestamp();
    if (st.alloc_profile) profReport();
    if (st.gc_verbose) {
        var buf: [256]u8 = undefined;
        const us = std.time.ns_per_us;
        const s = std.fmt.bufPrint(&buf, "[gc #{d}: live {d} MiB, heap {d} MiB (slabs {d}, large {d}), next in {d} MiB; stop {d} us, finish-sweep {d} us, roots {d} us, mark {d} us, sweep {d} us; total GC time {d} ms]\n", .{
            st.gc_count,                                         live >> 20,                              heapBytes() >> 20,
            st.slab_pool.committed_bytes.load(.monotonic) >> 20, st.large_bytes.load(.monotonic) >> 20, next >> 20,
            st.gc_sync_ns / us,                                  st.gc_finish_ns / us,
            @as(u64, @intCast(roots_ns - finish_ns)) / us,        st.gc_mark_ns / us,
            st.gc_sweep_ns / us,                                 st.gc_total_ns / 1_000_000,
        }) catch "";
        writeFd(2, s);
        var wbuf: [384]u8 = undefined;
        const why = GC_WHY_NAMES[@min(gc_why, GC_WHY_NAMES.len - 1)];
        const w = if (gc_why == 5)
            std.fmt.bufPrint(&wbuf, "[gc #{d} why: {s} from {s}.{s}; explicit requests skipped so far: {d}]\n", .{
                st.gc_count, why, if (st.gc_caller) |m| m.class else "?", if (st.gc_caller) |m| m.name else "?", st.gc_explicit_skipped,
            }) catch ""
        else
            std.fmt.bufPrint(&wbuf, "[gc #{d} why: {s}; explicit requests skipped so far: {d}]\n", .{ st.gc_count, why, st.gc_explicit_skipped }) catch "";
        writeFd(2, w);
        gc_why = 0;
    }
}

const CensusEnt = struct { c: *Class, count: u64, bytes: u64 };

/// Live bytes by class (MONO_HEAP_CENSUS): walks every slab and large object right after the mark phase, world stopped.
fn heapCensus() void {
    var map: std.AutoHashMapUnmanaged(*Class, [2]u64) = .empty;
    defer map.deinit(gpa);
    var total: u64 = 0;
    var it = st.slab_pool.slab_set.keyIterator();
    while (it.next()) |base| {
        const b: *mem_pool.SlabBlock = @ptrFromInt(base.*);
        if (b.slot_size == 0 or b.nslots == 0) continue;
        var i: u32 = 0;
        while (i < b.nslots) : (i += 1) {
            const o: *Object = @ptrCast(@alignCast(b.data + mem_pool.SLAB_HDR_BYTES + @as(usize, i) * b.slot_size));
            if (o.gc_bits & (MARK | ALLOC | ARENA) != (MARK | ALLOC)) continue;
            const e = map.getOrPut(gpa, o.class) catch return;
            if (!e.found_existing) e.value_ptr.* = .{ 0, 0 };
            e.value_ptr.*[0] += 1;
            e.value_ptr.*[1] += b.slot_size;
            total += b.slot_size;
        }
    }
    for (st.large_index.items) |o| {
        if (o.gc_bits & (MARK | ALLOC | ARENA) != (MARK | ALLOC)) continue;
        const sz: u64 = objectSize(o) + @sizeOf(LargeHdr);
        const e = map.getOrPut(gpa, o.class) catch return;
        if (!e.found_existing) e.value_ptr.* = .{ 0, 0 };
        e.value_ptr.*[0] += 1;
        e.value_ptr.*[1] += sz;
        total += sz;
    }
    var list: std.ArrayList(CensusEnt) = .empty;
    defer list.deinit(gpa);
    var mit = map.iterator();
    while (mit.next()) |e| list.append(gpa, .{ .c = e.key_ptr.*, .count = e.value_ptr.*[0], .bytes = e.value_ptr.*[1] }) catch return;
    std.mem.sort(CensusEnt, list.items, {}, struct {
        fn gt(_: void, a: CensusEnt, b: CensusEnt) bool {
            return a.bytes > b.bytes;
        }
    }.gt);
    var buf: [384]u8 = undefined;
    writeFd(2, std.fmt.bufPrint(&buf, "[heap census #{d}: {d} MiB live in {d} classes]\n", .{ st.gc_count, total >> 20, list.items.len }) catch "");
    for (list.items[0..@min(30, list.items.len)]) |e| {
        writeFd(2, std.fmt.bufPrint(&buf, "  {d:>6} MiB {d:>9} obj  {s}\n", .{ e.bytes >> 20, e.count, e.c.name }) catch "");
    }
}

fn sweepList(head: ?*Object, live: *usize) ?*Object {
    var new_head: ?*Object = null;
    var o = head;
    var dead: std.ArrayList(u32) = .empty;
    defer dead.deinit(gpa);
    while (o) |obj| {
        const nx = largeNext(obj).*;
        if (obj.gc_bits & MARK != 0) {
            obj.gc_bits &= ~MARK;
            largeNext(obj).* = new_head;
            new_head = obj;
            live.* += objectSize(obj);
        } else {
            const mi = obj.gc_bits >> MON_SHIFT;
            if (mi != 0) dead.append(gpa, mi) catch @panic("GC: out of memory");
            freeObject(obj);
        }
        o = nx;
    }
    releaseMonitors(dead.items);
    return new_head;
}

/// Frame boundary hook of the client renderer: counts frames (epochs). Collection is driven by allocation.
pub fn beginFrame() u64 {
    return st.frame_epoch.fetchAdd(1, .acq_rel) + 1;
}

/// End a render/tick frame: resets the caller thread's transient frame arena in O(1).
pub fn endFrame() void {
    if (cur.use_arena_depth == 0) {
        cur.scratch_arena.resetRetainingCapacity();
    }
}

/// Temporarily leave the calling thread's arena scope. Everything allocated between
/// `arenaSuspend()` and `arenaResume()` goes to the persistent SlabPool. Required around code that
/// publishes objects into process-global state (intern table, class mirrors, static initializers):
/// an object placed in the scope arena is rewound by `popArenaScope`, so a global reference to it
/// would dangle (use-after-rewind).
pub inline fn arenaSuspend() u32 {
    const d = cur.use_arena_depth;
    cur.use_arena_depth = 0;
    return d;
}

pub inline fn arenaResume(depth: u32) void {
    cur.use_arena_depth = depth;
}

/// Enter a zero-free scoped ArenaAllocator region on the calling thread (`allocObject` & temporary
/// buffers bump linearly inside thread-local 256 KiB slabs until `popArenaScope`).
/// CONTRACT: objects allocated inside the scope must not be reachable after `popArenaScope`
/// (no static/field stores into longer-lived objects, no thread hand-off). The runtime itself keeps
/// the intern table, class mirrors and class initialization out of the scope (`arenaSuspend`).
pub inline fn arenaMark() mem_pool.ArenaMark {
    return cur.scratch_arena.mark();
}

pub inline fn arenaRestore(mark: mem_pool.ArenaMark) void {
    cur.scratch_arena.restore(mark);
}

pub inline fn pushArenaScope() mem_pool.ArenaMark {
    cur.use_arena_depth += 1;
    return cur.scratch_arena.mark();
}

pub inline fn popArenaScope(mark: mem_pool.ArenaMark) void {
    cur.scratch_arena.restore(mark);
    cur.use_arena_depth -|= 1;
}

/// Lazy-start the low-power worker pool (`workers`) once per process.
pub fn ensureWorkers() *thread_pool.WorkerPool {
    if (st.workers_started.load(.acquire)) return &st.workers;
    st.workers_lock.lock();
    defer st.workers_lock.unlock();
    if (!st.workers_started.load(.monotonic)) {
        st.workers.start(0) catch @panic("monolith: failed to start worker pool");
        st.workers_started.store(true, .release);
    }
    return &st.workers;
}

fn freeObject(o: *Object) void {
    const size = objectSize(o) + @sizeOf(LargeHdr);
    const p: [*]align(16) u8 = @ptrFromInt(@intFromPtr(o) - @sizeOf(LargeHdr));
    _ = st.large_bytes.fetchSub(size, .monotonic);
    gpa.free(p[0..size]);
}

// ------------------------------------------------------------------ frames / stack
pub inline fn pushFrame(f: *Frame) E!void {
    _ = try pushFrameT(f);
}

pub inline fn pushFrameT(f: *Frame) E!*ThreadState {
    const t = cur;
    f.prev = t.top;
    t.top = f;
    // One compare covers the stack-overflow check and the method-entry safepoint poll: a collector that wants
    // the world stopped raises every thread's poll_limit to maxInt.
    if (@frameAddress() < @atomicLoad(usize, &t.poll_limit, .monotonic)) try entrySlow(f);
    return t;
}

noinline fn entrySlow(f: *Frame) E!void {
    const t = cur;
    if (@intFromPtr(f) < t.stack_limit) {
        t.top = f.prev;
        return stackOverflow();
    }
    safepoint();
}

pub inline fn popFrame(f: *Frame) void {
    cur.top = f.prev;
}

pub inline fn popFrameT(t: *ThreadState, f: *Frame) void {
    t.top = f.prev;
}

fn stackOverflow() E {
    const saved = cur.stack_limit;
    cur.stack_limit -|= 192 * 1024; // let the error be constructed
    @atomicStore(usize, &cur.poll_limit, cur.stack_limit, .monotonic);
    defer {
        cur.stack_limit = saved;
        @atomicStore(usize, &cur.poll_limit, if (gc_requested.load(.seq_cst)) std.math.maxInt(usize) else saved, .monotonic);
    }
    return throwKind(.StackOverflow, null);
}

// ------------------------------------------------------------------ exceptions
threadlocal var throw_depth: u32 = 0;

pub fn throwKind(k: Exc, msg: ?[]const u8) E {
    // Constructing a runtime exception must not itself keep failing (e.g. a class library
    // that does not match the JDK the program was compiled against): fail loudly instead.
    throw_depth += 1;
    defer throw_depth -= 1;
    if (throw_depth > 8) {
        var buf: [512]u8 = undefined;
        writeFd(2, std.fmt.bufPrint(&buf, "monolith: fatal: recursive failure while creating {s}: {s}\n", .{ @tagName(k), msg orelse "" }) catch "monolith: fatal: recursive exception\n");
        hardExit(134);
    }
    const ex = hooks.make_exception(k, msg) catch return error.Throw;
    cur.exc = ex;
    return error.Throw;
}

pub fn throwKindNR(k: Exc, msg: ?[]const u8) E!noreturn {
    return throwKind(k, msg);
}

pub fn athrow(r: Ref) E!noreturn {
    if (r == null) return throwKind(.NPE, null);
    cur.exc = r;
    return error.Throw;
}

pub inline fn pending() *Object {
    return cur.exc.?;
}

pub inline fn takePending() Ref {
    const e = cur.exc;
    cur.exc = null;
    return e;
}

pub inline fn nn(r: Ref) E!*Object {
    return r orelse npeCold();
}

noinline fn npeCold() E {
    return throwKind(.NPE, null);
}

pub fn missingClass(name: []const u8) E!noreturn {
    return throwKind(.NoClassDefFound, name);
}

pub fn unsupported(msg: []const u8) E!noreturn {
    return throwKind(.UnsupportedOperation, msg);
}

pub fn unsatisfiedLink(msg: []const u8) E!noreturn {
    return throwKind(.UnsatisfiedLink, msg);
}

pub fn abstractMethod(msg: []const u8) E!noreturn {
    return throwKind(.AbstractMethod, msg);
}

/// vtable / itable entry for a method that RTA proved unreachable.
pub fn prunedMethod() callconv(.c) noreturn {
    @panic("monolith: RTA-pruned method was invoked (static analysis unsoundness, e.g. reflection)");
}
pub const pruned: *const anyopaque = @ptrCast(&prunedMethod);


// ------------------------------------------------------------------ JDK intrinsics (see INTRINSICS in monolith/zigen.py)
pub inline fn iFloor(x: f64) f64 {
    return @floor(x);
}
pub inline fn iCeil(x: f64) f64 {
    return @ceil(x);
}
pub inline fn iSqrt(x: f64) f64 {
    return @sqrt(x);
}
pub inline fn iAbsI(a: i32) i32 {
    return if (a < 0) 0 -% a else a;
}
pub inline fn iAbsJ(a: i64) i64 {
    return if (a < 0) 0 -% a else a;
}
pub inline fn iAbsF(a: f32) f32 {
    return @bitCast(@as(u32, @bitCast(a)) & 0x7fffffff);
}
pub inline fn iAbsD(a: f64) f64 {
    return @bitCast(@as(u64, @bitCast(a)) & 0x7fffffffffffffff);
}
pub inline fn iMinI(a: i32, b: i32) i32 {
    return if (a <= b) a else b;
}
pub inline fn iMaxI(a: i32, b: i32) i32 {
    return if (a >= b) a else b;
}
pub inline fn iMinJ(a: i64, b: i64) i64 {
    return if (a <= b) a else b;
}
pub inline fn iMaxJ(a: i64, b: i64) i64 {
    return if (a >= b) a else b;
}
pub inline fn iMinF(a: f32, b: f32) f32 {
    if (a != a) return a;
    if (a == 0 and b == 0 and @as(u32, @bitCast(b)) == 0x80000000) return b;
    return if (a <= b) a else b;
}
pub inline fn iMaxF(a: f32, b: f32) f32 {
    if (a != a) return a;
    if (a == 0 and b == 0 and @as(u32, @bitCast(a)) == 0x80000000) return b;
    return if (a >= b) a else b;
}
pub inline fn iMinD(a: f64, b: f64) f64 {
    if (a != a) return a;
    if (a == 0 and b == 0 and @as(u64, @bitCast(b)) == 0x8000000000000000) return b;
    return if (a <= b) a else b;
}
pub inline fn iMaxD(a: f64, b: f64) f64 {
    if (a != a) return a;
    if (a == 0 and b == 0 and @as(u64, @bitCast(a)) == 0x8000000000000000) return b;
    return if (a >= b) a else b;
}
pub fn iFloorDivI(x: i32, y: i32) E!i32 {
    const q = try idiv(x, y);
    if ((x ^ y) >= 0 or q *% y == x) return q;
    return q -% 1;
}
pub fn iFloorModI(x: i32, y: i32) E!i32 {
    const m = try irem(x, y);
    if ((m ^ y) >= 0 or m == 0) return m;
    return m +% y;
}
pub fn iFloorDivJ(x: i64, y: i64) E!i64 {
    const q = try ldiv(x, y);
    if ((x ^ y) >= 0 or q *% y == x) return q;
    return q -% 1;
}
pub fn iFloorModJ(x: i64, y: i64) E!i64 {
    const m = try lrem(x, y);
    if ((m ^ y) >= 0 or m == 0) return m;
    return m +% y;
}
pub inline fn iExpD(a: f64) i32 {
    return @as(i32, @intCast((@as(u64, @bitCast(a)) >> 52) & 0x7ff)) - 1023;
}
pub inline fn iExpF(a: f32) i32 {
    return @as(i32, @intCast((@as(u32, @bitCast(a)) >> 23) & 0xff)) - 127;
}
pub inline fn iCmpI(a: i32, b: i32) i32 {
    return if (a < b) -1 else if (a == b) 0 else 1;
}
pub inline fn iCmpJ(a: i64, b: i64) i32 {
    return if (a < b) -1 else if (a == b) 0 else 1;
}
pub inline fn iSigI(a: i32) i32 {
    return if (a > 0) 1 else if (a < 0) -1 else 0;
}
pub inline fn iSigJ(a: i64) i32 {
    return if (a > 0) 1 else if (a < 0) -1 else 0;
}
pub inline fn iRotlI(a: i32, n: i32) i32 {
    return @bitCast(std.math.rotl(u32, @bitCast(a), @as(u32, @bitCast(n)) & 31));
}
pub inline fn iRotrI(a: i32, n: i32) i32 {
    return @bitCast(std.math.rotr(u32, @bitCast(a), @as(u32, @bitCast(n)) & 31));
}
pub inline fn iRotlJ(a: i64, n: i32) i64 {
    return @bitCast(std.math.rotl(u64, @bitCast(a), @as(u32, @bitCast(n)) & 63));
}
pub inline fn iRotrJ(a: i64, n: i32) i64 {
    return @bitCast(std.math.rotr(u64, @bitCast(a), @as(u32, @bitCast(n)) & 63));
}
pub inline fn iIdI(a: i32) i32 {
    return a;
}
pub inline fn iHashJ(a: i64) i32 {
    const u: u64 = @bitCast(a);
    return @bitCast(@as(u32, @truncate(u ^ (u >> 32))));
}
pub inline fn iToUnsignedLong(a: i32) i64 {
    return @as(i64, @as(u32, @bitCast(a)));
}
pub inline fn iBitCountI(a: i32) i32 {
    return @popCount(@as(u32, @bitCast(a)));
}
pub inline fn iBitCountJ(a: i64) i32 {
    return @popCount(@as(u64, @bitCast(a)));
}
pub inline fn iClzI(a: i32) i32 {
    return @clz(@as(u32, @bitCast(a)));
}
pub inline fn iClzJ(a: i64) i32 {
    return @clz(@as(u64, @bitCast(a)));
}
pub inline fn iCtzI(a: i32) i32 {
    return @ctz(@as(u32, @bitCast(a)));
}
pub inline fn iCtzJ(a: i64) i32 {
    return @ctz(@as(u64, @bitCast(a)));
}
pub inline fn iF2I(a: f32) i32 {
    return @bitCast(a);
}
pub inline fn iI2F(a: i32) f32 {
    return @bitCast(a);
}
pub inline fn iD2J(a: f64) i64 {
    return @bitCast(a);
}
pub inline fn iJ2D(a: i64) f64 {
    return @bitCast(a);
}
pub inline fn iCopySignD(mag: f64, sign: f64) f64 {
    return std.math.copysign(mag, sign);
}
pub inline fn iCopySignF(mag: f32, sign: f32) f32 {
    return std.math.copysign(mag, sign);
}
pub inline fn iSignumD(a: f64) f64 {
    if (a == 0.0 or a != a) return a;
    return if (a > 0.0) 1.0 else -1.0;
}
pub inline fn iSignumF(a: f32) f32 {
    if (a == 0.0 or a != a) return a;
    return if (a > 0.0) 1.0 else -1.0;
}
pub inline fn iFmaD(a: f64, b: f64, c: f64) f64 {
    return @mulAdd(f64, a, b, c);
}
pub inline fn iFmaF(a: f32, b: f32, c: f32) f32 {
    return @mulAdd(f32, a, b, c);
}
pub inline fn iRevI(a: i32) i32 {
    return @bitCast(@bitReverse(@as(u32, @bitCast(a))));
}
pub inline fn iRevJ(a: i64) i64 {
    return @bitCast(@bitReverse(@as(u64, @bitCast(a))));
}
pub inline fn iRevBytesI(a: i32) i32 {
    return @bitCast(@byteSwap(@as(u32, @bitCast(a))));
}
pub inline fn iRevBytesJ(a: i64) i64 {
    return @bitCast(@byteSwap(@as(u64, @bitCast(a))));
}
pub inline fn iRevBytesS(a: i32) i32 {
    return @as(i32, @as(i16, @bitCast(@byteSwap(@as(u16, @truncate(@as(u32, @bitCast(a))))))));
}
pub inline fn iRevBytesC(a: i32) i32 {
    return @as(i32, @byteSwap(@as(u16, @truncate(@as(u32, @bitCast(a))))));
}
pub inline fn iCmpUI(a: i32, b: i32) i32 {
    const ua: u32 = @bitCast(a);
    const ub: u32 = @bitCast(b);
    return if (ua < ub) -1 else if (ua == ub) 0 else 1;
}
pub inline fn iCmpUJ(a: i64, b: i64) i32 {
    const ua: u64 = @bitCast(a);
    const ub: u64 = @bitCast(b);
    return if (ua < ub) -1 else if (ua == ub) 0 else 1;
}
pub inline fn iDivUI(a: i32, b: i32) E!i32 {
    if (b == 0) return throwKind(.Arithmetic, "/ by zero");
    return @bitCast(@as(u32, @bitCast(a)) / @as(u32, @bitCast(b)));
}
pub inline fn iRemUI(a: i32, b: i32) E!i32 {
    if (b == 0) return throwKind(.Arithmetic, "/ by zero");
    return @bitCast(@as(u32, @bitCast(a)) % @as(u32, @bitCast(b)));
}
pub inline fn iDivUJ(a: i64, b: i64) E!i64 {
    if (b == 0) return throwKind(.Arithmetic, "/ by zero");
    return @bitCast(@as(u64, @bitCast(a)) / @as(u64, @bitCast(b)));
}
pub inline fn iRemUJ(a: i64, b: i64) E!i64 {
    if (b == 0) return throwKind(.Arithmetic, "/ by zero");
    return @bitCast(@as(u64, @bitCast(a)) % @as(u64, @bitCast(b)));
}
pub inline fn iIsNanF(a: f32) i32 {
    return @intFromBool(std.math.isNan(a));
}
pub inline fn iIsNanD(a: f64) i32 {
    return @intFromBool(std.math.isNan(a));
}
pub inline fn iIsInfF(a: f32) i32 {
    return @intFromBool(std.math.isInf(a));
}
pub inline fn iIsInfD(a: f64) i32 {
    return @intFromBool(std.math.isInf(a));
}
pub inline fn iIsFinF(a: f32) i32 {
    return @intFromBool(std.math.isFinite(a));
}
pub inline fn iIsFinD(a: f64) i32 {
    return @intFromBool(std.math.isFinite(a));
}
pub inline fn iFloatToIntBits(a: f32) i32 {
    return if (std.math.isNan(a)) 0x7fc00000 else @bitCast(a);
}
pub inline fn iDoubleToLongBits(a: f64) i64 {
    return if (std.math.isNan(a)) 0x7ff8000000000000 else @bitCast(a);
}

// ------------------------------------------------------------------ arithmetic (JVMS semantics)
pub fn idiv(a: i32, b: i32) callconv(hot) E!i32 {
    if (b == 0) return throwKind(.Arithmetic, "/ by zero");
    if (b == -1) return 0 -% a;
    return @divTrunc(a, b);
}
pub fn irem(a: i32, b: i32) callconv(hot) E!i32 {
    if (b == 0) return throwKind(.Arithmetic, "/ by zero");
    if (b == -1) return 0;
    return @rem(a, b);
}
pub fn ldiv(a: i64, b: i64) callconv(hot) E!i64 {
    if (b == 0) return throwKind(.Arithmetic, "/ by zero");
    if (b == -1) return 0 -% a;
    return @divTrunc(a, b);
}
pub fn lrem(a: i64, b: i64) callconv(hot) E!i64 {
    if (b == 0) return throwKind(.Arithmetic, "/ by zero");
    if (b == -1) return 0;
    return @rem(a, b);
}
pub fn ishl(a: i32, b: i32) callconv(hot) i32 {
    return a << @as(u5, @intCast(b & 31));
}
pub fn ishr(a: i32, b: i32) callconv(hot) i32 {
    return a >> @as(u5, @intCast(b & 31));
}
pub fn iushr(a: i32, b: i32) callconv(hot) i32 {
    return @bitCast(@as(u32, @bitCast(a)) >> @as(u5, @intCast(b & 31)));
}
pub fn lshl(a: i64, b: i32) callconv(hot) i64 {
    return a << @as(u6, @intCast(b & 63));
}
pub fn lshr(a: i64, b: i32) callconv(hot) i64 {
    return a >> @as(u6, @intCast(b & 63));
}
pub fn lushr(a: i64, b: i32) callconv(hot) i64 {
    return @bitCast(@as(u64, @bitCast(a)) >> @as(u6, @intCast(b & 63)));
}
pub fn fmod32(a: f32, b: f32) f32 {
    return @rem(a, b);
}
pub fn fmod64(a: f64, b: f64) f64 {
    return @rem(a, b);
}
pub fn f2i(v: f32) callconv(hot) i32 {
    if (std.math.isNan(v)) return 0;
    if (v >= 2147483647.0) return std.math.maxInt(i32);
    if (v <= -2147483648.0) return std.math.minInt(i32);
    return @intFromFloat(v);
}
pub fn d2i(v: f64) callconv(hot) i32 {
    if (std.math.isNan(v)) return 0;
    if (v >= 2147483647.0) return std.math.maxInt(i32);
    if (v <= -2147483648.0) return std.math.minInt(i32);
    return @intFromFloat(v);
}
pub fn f2l(v: f32) callconv(hot) i64 {
    if (std.math.isNan(v)) return 0;
    if (v >= 9223372036854775807.0) return std.math.maxInt(i64);
    if (v <= -9223372036854775808.0) return std.math.minInt(i64);
    return @intFromFloat(v);
}
pub fn d2l(v: f64) callconv(hot) i64 {
    if (std.math.isNan(v)) return 0;
    if (v >= 9223372036854775807.0) return std.math.maxInt(i64);
    if (v <= -9223372036854775808.0) return std.math.minInt(i64);
    return @intFromFloat(v);
}
pub fn i2b(v: i32) callconv(hot) i32 {
    return @as(i8, @truncate(v));
}
pub fn i2c(v: i32) callconv(hot) i32 {
    return @as(u16, @truncate(@as(u32, @bitCast(v))));
}
pub fn i2s(v: i32) callconv(hot) i32 {
    return @as(i16, @truncate(v));
}
pub fn lcmp(a: i64, b: i64) callconv(hot) i32 {
    return if (a < b) -1 else if (a > b) @as(i32, 1) else 0;
}
pub fn fcmpl(a: f32, b: f32) callconv(hot) i32 {
    return if (a > b) @as(i32, 1) else if (a == b) @as(i32, 0) else -1;
}
pub fn fcmpg(a: f32, b: f32) callconv(hot) i32 {
    return if (a < b) @as(i32, -1) else if (a == b) @as(i32, 0) else 1;
}
pub fn dcmpl(a: f64, b: f64) callconv(hot) i32 {
    return if (a > b) @as(i32, 1) else if (a == b) @as(i32, 0) else -1;
}
pub fn dcmpg(a: f64, b: f64) callconv(hot) i32 {
    return if (a < b) @as(i32, -1) else if (a == b) @as(i32, 0) else 1;
}
pub fn z(v: i32) callconv(hot) i8 {
    return @intCast(v & 1);
}
pub fn b8(v: i32) callconv(hot) i8 {
    return @truncate(v);
}
pub fn c16(v: i32) callconv(hot) u16 {
    return @truncate(@as(u32, @bitCast(v)));
}
pub fn s16(v: i32) callconv(hot) i16 {
    return @truncate(v);
}

// The self-hosted backend does not inline: force it for the tiny hot helpers. Under LLVM the
// optimizer decides (forcing inlining there measured 3.5x slower on allocation-heavy code).
const hot: std.builtin.CallingConvention = if (@import("builtin").zig_backend == .stage2_llvm) .auto else .@"inline";

// ------------------------------------------------------------------ classes / type checks
pub inline fn ensureInit(c: *Class) E!void {
    if (@atomicLoad(u8, &c.init_state, .acquire) != INIT_DONE) try initClass(c);
}


pub fn initClass(c: *Class) E!void {
    lockSafe(init_lock);
    while (true) {
        switch (c.init_state) {
            INIT_DONE => {
                init_lock.unlock();
                return;
            },
            INIT_ERROR => {
                init_lock.unlock();
                const ex = hooks.make_ncdfe(c) catch return error.Throw;
                cur.exc = ex;
                return error.Throw;
            },
            INIT_RUNNING => {
                if (c.init_thread == cur) {
                    init_lock.unlock();
                    return;
                }
                if (traceInit()) {
                    writeFd(2, "monolith: waiting for the initialization of ");
                    writeFd(2, c.name);
                    var tb: [96]u8 = undefined;
                    writeFd(2, std.fmt.bufPrint(&tb, " held by thread {x}, caller {x}\n", .{ @intFromPtr(c.init_thread), @intFromPtr(cur) }) catch " held by another thread\n");
                }
                enterSafe();
                init_cond.wait(init_lock);
                init_lock.unlock();
                leaveSafe();
                lockSafe(init_lock);
            },
            else => {
                c.init_state = INIT_RUNNING;
                c.init_thread = cur;
                break;
            },
        }
    }
    init_lock.unlock();
    // static initializers publish objects into static fields: never inside a scoped arena
    const arena_depth = arenaSuspend();
    defer arenaResume(arena_depth);
    var failed = false;
    if (c.kind == .instance) {
        if (c.super) |s| {
            ensureInit(s) catch {
                failed = true;
            };
        }
        // JLS 12.4.2 step 7: superinterfaces that declare default methods are initialized too
        if (!failed) {
            for (c.interfaces) |ic| {
                if (ic.has_default) {
                    ensureInit(ic) catch {
                        failed = true;
                        break;
                    };
                }
            }
        }
    }
    if (!failed) {
        if (c.clinit) |f| {
            f() catch {
                const ex = cur.exc;
                if (traceInit()) {
                    writeFd(2, "monolith: static initializer failed: ");
                    writeFd(2, c.name);
                    writeFd(2, "\n");
                    cur.exc = null;
                    hooks.thread_uncaught(null, ex) catch {};
                    cur.exc = ex;
                }
                c.init_error = ex;
                c.init_error_thread = cur.java_thread;
                if (!hooks.is_error(ex)) {
                    cur.exc = null;
                    const w = hooks.wrap_init_error(ex) catch null;
                    if (w != null) cur.exc = w;
                }
                failed = true;
            };
        }
    }
    lockSafe(init_lock);
    @atomicStore(u8, &c.init_state, if (failed) INIT_ERROR else INIT_DONE, .release);
    c.init_thread = null;
    init_cond.broadcast();
    init_lock.unlock();
    if (failed) return error.Throw;
}

/// MONO_TRACE_INIT=1 prints the exception of every failing static initializer (diagnostics).
fn traceInit() bool {
    if (st.trace_init_cached < 0) st.trace_init_cached = if (getenv("MONO_TRACE_INIT") != null) 1 else 0;
    return st.trace_init_cached == 1;
}

pub fn isSubclass(c: *Class, t: *Class) bool {
    if (c == t) return true;
    switch (t.kind) {
        .interface => {
            if (c.kind == .array) return t == hooks.cloneable or t == hooks.serializable;
            for (c.interfaces) |i| if (i == t) return true;
            return false;
        },
        .array => {
            if (c.kind != .array) return false;
            const ce = c.elem.?;
            const te = t.elem.?;
            if (ce.kind == .primitive or te.kind == .primitive) return ce == te;
            return isSubclass(ce, te);
        },
        .instance => {
            // arrays and interfaces (whose `super` is null) are subtypes of Object
            if (t == hooks.object_class) return c.kind != .primitive;
            if (c.kind == .array) return false;
            var s = c.super;
            while (s) |x| : (s = x.super) if (x == t) return true;
            return false;
        },
        .primitive => return false,
    }
}

pub inline fn instanceOf(r: Ref, t: *Class) bool {
    const o = r orelse return false;
    return isSubclass(o.class, t);
}

pub fn checkcast(r: Ref, t: *Class) callconv(hot) E!void {
    const o = r orelse return;
    if (o.class != t and !isSubclass(o.class, t)) return castFail(o, t);
}

noinline fn castFail(o: *Object, t: *Class) E!void {
    var buf: [512]u8 = undefined;
    const msg = std.fmt.bufPrint(&buf, "class {s} cannot be cast to class {s}", .{ dotted(o.class.name, buf[256..]), t.name }) catch "class cast";
    return throwKind(.ClassCast, msg);
}

fn dotted(name: []const u8, buf: []u8) []const u8 {
    const n = @min(name.len, buf.len);
    for (name[0..n], 0..) |ch, i| buf[i] = if (ch == '/') '.' else ch;
    return buf[0..n];
}

/// Interface dispatch with a per-call-site hint: the position of `iface` in the receiver's itable is
/// remembered and verified before use (a stale or racy hint is only a miss, never a wrong target).
pub inline fn ilookupH(r: *Object, iface: *Class, idx: usize, hint: *u16) E!*const anyopaque {
    const it = r.class.itable;
    const h = hint.*;
    if (h < it.len and it[h].iface == iface) return it[h].methods[idx];
    return ilookupSlow(r, iface, idx, hint);
}

noinline fn ilookupSlow(r: *Object, iface: *Class, idx: usize, hint: *u16) E!*const anyopaque {
    const it = r.class.itable;
    for (it, 0..) |e, k| {
        if (e.iface == iface) {
            hint.* = @intCast(k);
            return e.methods[idx];
        }
    }
    return throwKind(.IncompatibleClassChange, iface.name);
}

/// checkcast with a per-site memo of the last receiver class that passed.
pub inline fn checkcastH(r: Ref, t: *Class, last: *?*Class) E!void {
    const o = r orelse return;
    const c = o.class;
    if (c == t or c == @atomicLoad(?*Class, last, .monotonic)) return;
    return checkcastSlow(o, t, last);
}

noinline fn checkcastSlow(o: *Object, t: *Class, last: *?*Class) E!void {
    const c = o.class;
    if (!isSubclass(c, t)) return castFail(o, t);
    @atomicStore(?*Class, last, c, .monotonic);
}

pub inline fn instanceOfH(r: Ref, t: *Class, last: *?*Class) bool {
    const o = r orelse return false;
    const c = o.class;
    if (c == t or c == @atomicLoad(?*Class, last, .monotonic)) return true;
    if (!isSubclass(c, t)) return false;
    @atomicStore(?*Class, last, c, .monotonic);
    return true;
}

pub fn ilookup(r: *Object, iface: *Class, idx: usize) callconv(hot) E!*const anyopaque {
    for (r.class.itable) |e| {
        if (e.iface == iface) return e.methods[idx];
    }
    return throwKind(.IncompatibleClassChange, iface.name);
}

pub inline fn vtableOf(o: *Object) []const *const anyopaque {
    return o.class.vtable;
}

// primitive classes
fn primClass(comptime name: []const u8, comptime p: u8, comptime size: u32) Class {
    return .{ .name = name, .kind = .primitive, .prim = p, .elem_size = size, .init_state = INIT_DONE };
}
const prim_classes = [_]*Class{ prim_Z, prim_B, prim_C, prim_S, prim_I, prim_J, prim_F, prim_D, prim_V };

pub fn primByChar(ch: u8) *Class {
    return switch (ch) {
        'Z' => prim_Z,
        'B' => prim_B,
        'C' => prim_C,
        'S' => prim_S,
        'I' => prim_I,
        'J' => prim_J,
        'F' => prim_F,
        'D' => prim_D,
        else => prim_V,
    };
}


/// The array class whose component type is `elem` (created on demand, never freed).
/// Array class with element class `elem` (created on first use). Inline fast path: every array allocation
/// in generated code asks for its class.
pub inline fn arrayOf(elem: *Class) *Class {
    if (@atomicLoad(?*Class, &elem.array_class, .acquire)) |a| return a;
    return arrayOfSlow(elem);
}

noinline fn arrayOfSlow(elem: *Class) *Class {
    lockSafe(array_class_lock);
    defer array_class_lock.unlock();
    if (elem.array_class) |a| return a;
    const a = gpa.create(Class) catch @panic("out of memory");
    const nm = std.mem.concat(gpa, u8, &.{ "[", descriptorOf(elem) }) catch @panic("out of memory");
    a.* = .{
        .name = nm,
        .kind = .array,
        .flags = 0x0411,
        .super = hooks.object_class,
        .interfaces = array_ifaces,
        .vtable = hooks.object_class.vtable,
        .elem = elem,
        .elem_size = if (elem.kind == .primitive) elem.elem_size else @sizeOf(Ref),
        .init_state = INIT_DONE,
    };
    dyn_classes_lock.lock();
    dyn_classes.append(gpa, a) catch @panic("out of memory");
    dyn_classes_lock.unlock();
    @atomicStore(?*Class, &elem.array_class, a, .release);
    return a;
}

fn descriptorOf(c: *Class) []const u8 {
    if (c.kind == .primitive) return @as(*const [1]u8, &c.prim);
    if (c.kind == .array) return c.name;
    return std.mem.concat(gpa, u8, &.{ "L", c.name, ";" }) catch @panic("out of memory");
}


// ------------------------------------------------------------------ class lookup by name
// Class.forName, reflection and JNI look classes up by internal name: a lazily built hash table instead of a scan of
// the whole class list (tens of thousands of classes) per lookup.
var class_map: ?*std.StringHashMapUnmanaged(*Class) = null;
var class_map_mu: std.Thread.Mutex = .{};

pub fn findClass(name: []const u8) ?*Class {
    var m = @atomicLoad(?*std.StringHashMapUnmanaged(*Class), &class_map, .acquire);
    if (m == null) {
        class_map_mu.lock();
        defer class_map_mu.unlock();
        m = class_map;
        if (m == null) {
            const nm = gpa.create(std.StringHashMapUnmanaged(*Class)) catch @panic("out of memory");
            nm.* = .{};
            nm.ensureTotalCapacity(gpa, @intCast(hooks.all_classes.len)) catch @panic("out of memory");
            // first class of a name wins, as in the linear scan
            for (hooks.all_classes) |c| {
                const e = nm.getOrPutAssumeCapacity(c.name);
                if (!e.found_existing) e.value_ptr.* = c;
            }
            @atomicStore(?*std.StringHashMapUnmanaged(*Class), &class_map, nm, .release);
            m = nm;
        }
    }
    return m.?.get(name);
}

// ------------------------------------------------------------------ objects & arrays
pub fn newObject(c: *Class) E!Ref {
    if (c.kind != .instance or c.flags & 0x0400 != 0) return throwKind(.Instantiation, c.name);
    try ensureInit(c);
    return try allocObject(c, c.size);
}

pub inline fn arrayData(comptime T: type, a: *Array) [*]T {
    return @ptrFromInt(@intFromPtr(a) + @sizeOf(Array));
}

pub inline fn asArray(o: *Object) *Array {
    return @ptrCast(@alignCast(o));
}

/// Scratch-arena object: rewound with its scope, never collected (header bit ARENA), not counted as heap.
inline fn allocArenaObject(c: *Class, size: usize) E!*Object {
    const t0 = cur;
    const end = @max(size, @sizeOf(Object));
    const raw = t0.scratch_arena.allocZeroed(end, 16) orelse return throwOOM();
    const o: *Object = @ptrCast(@alignCast(raw));
    o.class = c;
    o.gc_bits = ALLOC | ARENA;
    return o;
}

pub fn newObjectArena(c: *Class) E!Ref {
    if (c.kind != .instance or c.flags & 0x0400 != 0) return throwKind(.Instantiation, c.name);
    try ensureInit(c);
    return try allocArenaObject(c, c.size);
}

pub fn newArray(arr_class: *Class, len: i32) E!Ref {
    if (len < 0) {
        var buf: [32]u8 = undefined;
        return throwKind(.NegativeArraySize, std.fmt.bufPrint(&buf, "{d}", .{len}) catch null);
    }
    const size = @sizeOf(Array) + @as(usize, @intCast(len)) * arr_class.elem_size;
    const o = try allocObject(arr_class, size);
    asArray(o).length = len;
    return o;
}

pub fn newArrayArena(arr_class: *Class, len: i32) E!Ref {
    if (len < 0) {
        var buf: [32]u8 = undefined;
        return throwKind(.NegativeArraySize, std.fmt.bufPrint(&buf, "{d}", .{len}) catch null);
    }
    const size = @sizeOf(Array) + @as(usize, @intCast(len)) * arr_class.elem_size;
    const o = try allocArenaObject(arr_class, size);
    asArray(o).length = len;
    return o;
}

pub fn multiNewArray(arr_class: *Class, dims: []const i32) E!Ref {
    for (dims) |d| if (d < 0) {
        var buf: [32]u8 = undefined;
        return throwKind(.NegativeArraySize, std.fmt.bufPrint(&buf, "{d}", .{d}) catch null);
    };
    return multiRec(arr_class, dims);
}

fn multiRec(arr_class: *Class, dims: []const i32) E!Ref {
    const r = try newArray(arr_class, dims[0]);
    if (dims.len == 1) return r;
    var fr_refs = [_]Ref{r};
    var fr = Frame{ .refs = &fr_refs, .n = 1, .meth = &rt_meta };
    try pushFrame(&fr);
    defer popFrame(&fr);
    const a = asArray(r.?);
    var i: usize = 0;
    while (i < @as(usize, @intCast(dims[0]))) : (i += 1) {
        const sub = try multiRec(arr_class.elem.?, dims[1..]);
        arrayData(Ref, a)[i] = sub;
    }
    return r;
}
const rt_meta = MethodMeta{ .class = "<runtime>", .name = "<runtime>", .desc = "" };

noinline fn indexError(idx: i32, len: i32) E {
    var buf: [96]u8 = undefined;
    return throwKind(.ArrayIndex, std.fmt.bufPrint(&buf, "Index {d} out of bounds for length {d}", .{ idx, len }) catch null);
}

pub inline fn arrLen(r: Ref) E!i32 {
    return asArray(try nn(r)).length;
}

pub inline fn aload(comptime T: type, r: Ref, idx: i32) E!T {
    const a = asArray(try nn(r));
    if (@as(u32, @bitCast(idx)) >= @as(u32, @bitCast(a.length))) return indexError(idx, a.length);
    return arrayData(T, a)[@intCast(idx)];
}

pub inline fn astore(comptime T: type, r: Ref, idx: i32, v: T) E!void {
    const a = asArray(try nn(r));
    if (@as(u32, @bitCast(idx)) >= @as(u32, @bitCast(a.length))) return indexError(idx, a.length);
    arrayData(T, a)[@intCast(idx)] = v;
}

/// baload/bastore work on both byte[] and boolean[] (both 1 byte per element).
pub fn bastore(r: Ref, idx: i32, v: i32) E!void {
    const o = try nn(r);
    const val: i8 = if (o.class.elem.? == prim_Z) @intCast(v & 1) else @truncate(v);
    return astore(i8, r, idx, val);
}

pub fn aastore(r: Ref, idx: i32, v: Ref) E!void {
    const a = asArray(try nn(r));
    if (@as(u32, @bitCast(idx)) >= @as(u32, @bitCast(a.length))) return indexError(idx, a.length);
    if (v) |val| {
        if (!isSubclass(val.class, a.hdr.class.elem.?)) {
            var buf: [256]u8 = undefined;
            return throwKind(.ArrayStore, dotted(val.class.name, &buf));
        }
    }
    arrayData(Ref, a)[@intCast(idx)] = v;
}

// ------------------------------------------------------------------ lazy-frame leaf methods
// A method that cannot call, allocate, loop or catch only needs a shadow-stack frame when it throws (so that
// the stack trace still names it). The generated code uses these variants: the fast path is the plain check,
// the slow path pushes a real frame and runs the ordinary helper, which then builds the exception.
var leaf_refs: [1]Ref = .{null};

inline fn leafEnter(f: *Frame, meta: *const MethodMeta, line: u32) void {
    f.* = .{ .refs = &leaf_refs, .n = 0, .meth = meta, .line = line };
    f.prev = cur.top;
    cur.top = f;
}

noinline fn slowNn(r: Ref, meta: *const MethodMeta, line: u32) E!*Object {
    var F: Frame = undefined;
    leafEnter(&F, meta, line);
    defer cur.top = F.prev;
    return nn(r);
}
noinline fn slowArrLen(r: Ref, meta: *const MethodMeta, line: u32) E!i32 {
    var F: Frame = undefined;
    leafEnter(&F, meta, line);
    defer cur.top = F.prev;
    return arrLen(r);
}
noinline fn slowAload(comptime T: type, r: Ref, idx: i32, meta: *const MethodMeta, line: u32) E!T {
    var F: Frame = undefined;
    leafEnter(&F, meta, line);
    defer cur.top = F.prev;
    return aload(T, r, idx);
}
noinline fn slowAstore(comptime T: type, r: Ref, idx: i32, v: T, meta: *const MethodMeta, line: u32) E!void {
    var F: Frame = undefined;
    leafEnter(&F, meta, line);
    defer cur.top = F.prev;
    return astore(T, r, idx, v);
}
noinline fn slowBastore(r: Ref, idx: i32, v: i32, meta: *const MethodMeta, line: u32) E!void {
    var F: Frame = undefined;
    leafEnter(&F, meta, line);
    defer cur.top = F.prev;
    return bastore(r, idx, v);
}
noinline fn slowAastore(r: Ref, idx: i32, v: Ref, meta: *const MethodMeta, line: u32) E!void {
    var F: Frame = undefined;
    leafEnter(&F, meta, line);
    defer cur.top = F.prev;
    return aastore(r, idx, v);
}
noinline fn slowDiv(comptime T: type, comptime rem: bool, a: T, b: T, meta: *const MethodMeta, line: u32) E!T {
    var F: Frame = undefined;
    leafEnter(&F, meta, line);
    defer cur.top = F.prev;
    if (T == i32) return if (rem) irem(a, b) else idiv(a, b);
    return if (rem) lrem(a, b) else ldiv(a, b);
}

pub inline fn nnL(r: Ref, meta: *const MethodMeta, line: u32) E!*Object {
    return r orelse slowNn(r, meta, line);
}
pub inline fn arrLenL(r: Ref, meta: *const MethodMeta, line: u32) E!i32 {
    if (r) |o| return asArray(o).length;
    return slowArrLen(r, meta, line);
}
pub inline fn aloadL(comptime T: type, r: Ref, idx: i32, meta: *const MethodMeta, line: u32) E!T {
    if (r) |o| {
        const a = asArray(o);
        if (@as(u32, @bitCast(idx)) < @as(u32, @bitCast(a.length))) return arrayData(T, a)[@intCast(idx)];
    }
    return slowAload(T, r, idx, meta, line);
}
pub inline fn astoreL(comptime T: type, r: Ref, idx: i32, v: T, meta: *const MethodMeta, line: u32) E!void {
    if (r) |o| {
        const a = asArray(o);
        if (@as(u32, @bitCast(idx)) < @as(u32, @bitCast(a.length))) {
            arrayData(T, a)[@intCast(idx)] = v;
            return;
        }
    }
    return slowAstore(T, r, idx, v, meta, line);
}
pub inline fn bastoreL(r: Ref, idx: i32, v: i32, meta: *const MethodMeta, line: u32) E!void {
    if (r) |o| {
        const a = asArray(o);
        if (@as(u32, @bitCast(idx)) < @as(u32, @bitCast(a.length))) {
            arrayData(i8, a)[@intCast(idx)] = if (o.class.elem.? == prim_Z) @intCast(v & 1) else @truncate(v);
            return;
        }
    }
    return slowBastore(r, idx, v, meta, line);
}
pub inline fn aastoreL(r: Ref, idx: i32, v: Ref, meta: *const MethodMeta, line: u32) E!void {
    if (r) |o| {
        const a = asArray(o);
        if (@as(u32, @bitCast(idx)) < @as(u32, @bitCast(a.length))) {
            const ok = if (v) |val| (val.class == a.hdr.class.elem.? or isSubclass(val.class, a.hdr.class.elem.?)) else true;
            if (ok) {
                arrayData(Ref, a)[@intCast(idx)] = v;
                return;
            }
        }
    }
    return slowAastore(r, idx, v, meta, line);
}
// Checks for non-leaf methods: the frame's line is stored only on the throwing path (the original check is
// re-run there, so the exception is exactly the one the plain helper would throw).
noinline fn setLine(f: *Frame, line: u32) void {
    f.line = line;
}
pub inline fn nnF(r: Ref, f: *Frame, line: u32) E!*Object {
    if (r) |o| return o;
    setLine(f, line);
    return nn(r);
}
pub inline fn arrLenF(r: Ref, f: *Frame, line: u32) E!i32 {
    if (r) |o| return asArray(o).length;
    setLine(f, line);
    return arrLen(r);
}
pub inline fn aloadF(comptime T: type, r: Ref, idx: i32, f: *Frame, line: u32) E!T {
    if (r) |o| {
        const a = asArray(o);
        if (@as(u32, @bitCast(idx)) < @as(u32, @bitCast(a.length))) return arrayData(T, a)[@intCast(idx)];
    }
    setLine(f, line);
    return aload(T, r, idx);
}
pub inline fn astoreF(comptime T: type, r: Ref, idx: i32, v: T, f: *Frame, line: u32) E!void {
    if (r) |o| {
        const a = asArray(o);
        if (@as(u32, @bitCast(idx)) < @as(u32, @bitCast(a.length))) {
            arrayData(T, a)[@intCast(idx)] = v;
            return;
        }
    }
    setLine(f, line);
    return astore(T, r, idx, v);
}
pub inline fn bastoreF(r: Ref, idx: i32, v: i32, f: *Frame, line: u32) E!void {
    if (r) |o| {
        const a = asArray(o);
        if (@as(u32, @bitCast(idx)) < @as(u32, @bitCast(a.length))) {
            arrayData(i8, a)[@intCast(idx)] = if (o.class.elem.? == prim_Z) @intCast(v & 1) else @truncate(v);
            return;
        }
    }
    setLine(f, line);
    return bastore(r, idx, v);
}
pub inline fn aastoreF(r: Ref, idx: i32, v: Ref, f: *Frame, line: u32) E!void {
    if (r) |o| {
        const a = asArray(o);
        if (@as(u32, @bitCast(idx)) < @as(u32, @bitCast(a.length))) {
            const ok = if (v) |val| (val.class == a.hdr.class.elem.? or isSubclass(val.class, a.hdr.class.elem.?)) else true;
            if (ok) {
                arrayData(Ref, a)[@intCast(idx)] = v;
                return;
            }
        }
    }
    setLine(f, line);
    return aastore(r, idx, v);
}
pub inline fn idivF(a: i32, b: i32, f: *Frame, line: u32) E!i32 {
    if (b != 0 and b != -1) return @divTrunc(a, b);
    setLine(f, line);
    return idiv(a, b);
}
pub inline fn iremF(a: i32, b: i32, f: *Frame, line: u32) E!i32 {
    if (b != 0 and b != -1) return @rem(a, b);
    setLine(f, line);
    return irem(a, b);
}
pub inline fn ldivF(a: i64, b: i64, f: *Frame, line: u32) E!i64 {
    if (b != 0 and b != -1) return @divTrunc(a, b);
    setLine(f, line);
    return ldiv(a, b);
}
pub inline fn lremF(a: i64, b: i64, f: *Frame, line: u32) E!i64 {
    if (b != 0 and b != -1) return @rem(a, b);
    setLine(f, line);
    return lrem(a, b);
}
pub inline fn idivL(a: i32, b: i32, meta: *const MethodMeta, line: u32) E!i32 {
    if (b == 0) return slowDiv(i32, false, a, b, meta, line);
    if (b == -1) return 0 -% a;
    return @divTrunc(a, b);
}
pub inline fn iremL(a: i32, b: i32, meta: *const MethodMeta, line: u32) E!i32 {
    if (b == 0) return slowDiv(i32, true, a, b, meta, line);
    if (b == -1) return 0;
    return @rem(a, b);
}
pub inline fn ldivL(a: i64, b: i64, meta: *const MethodMeta, line: u32) E!i64 {
    if (b == 0) return slowDiv(i64, false, a, b, meta, line);
    if (b == -1) return 0 -% a;
    return @divTrunc(a, b);
}
pub inline fn lremL(a: i64, b: i64, meta: *const MethodMeta, line: u32) E!i64 {
    if (b == 0) return slowDiv(i64, true, a, b, meta, line);
    if (b == -1) return 0;
    return @rem(a, b);
}

pub fn cloneObject(r: Ref) E!Ref {
    const o = try nn(r);
    const size = objectSize(o);
    const c = o.class;
    if (c.kind != .array and !isSubclass(c, hooks.cloneable)) return throwKind(.CloneNotSupported, c.name);
    var fr_refs = [_]Ref{r};
    var fr = Frame{ .refs = &fr_refs, .n = 1, .meth = &rt_meta };
    try pushFrame(&fr);
    defer popFrame(&fr);
    const n = try allocObject(c, size);
    const src: [*]const u8 = @ptrCast(o);
    const dst: [*]u8 = @ptrCast(n);
    const hdr = @sizeOf(Object);
    @memcpy(dst[hdr..size], src[hdr..size]);
    return n;
}

pub fn arraycopy(src: Ref, sp: i32, dst: Ref, dp: i32, len: i32) E!void {
    const s = try nn(src);
    const d = try nn(dst);
    if (s.class.kind != .array or d.class.kind != .array) return throwKind(.ArrayStore, "arraycopy: argument type mismatch");
    const se = s.class.elem.?;
    const de = d.class.elem.?;
    const prim = se.kind == .primitive or de.kind == .primitive;
    if (prim and se != de) return throwKind(.ArrayStore, "arraycopy: type mismatch");
    const sa = asArray(s);
    const da = asArray(d);
    if (len < 0 or sp < 0 or dp < 0 or @as(i64, sp) + len > sa.length or @as(i64, dp) + len > da.length) {
        var buf: [128]u8 = undefined;
        const msg = if (sp < 0 or @as(i64, sp) + len > sa.length)
            std.fmt.bufPrint(&buf, "arraycopy: last source index {d} out of bounds for length {d}", .{ @as(i64, sp) + len, sa.length }) catch null
        else
            std.fmt.bufPrint(&buf, "arraycopy: last destination index {d} out of bounds for length {d}", .{ @as(i64, dp) + len, da.length }) catch null;
        if (len < 0) return throwKind(.ArrayIndex, std.fmt.bufPrint(&buf, "arraycopy: length {d} is negative", .{len}) catch null);
        return throwKind(.ArrayIndex, msg);
    }
    const es = s.class.elem_size;
    const sb: [*]u8 = @ptrCast(arrayData(u8, sa));
    const db: [*]u8 = @ptrCast(arrayData(u8, da));
    const n = @as(usize, @intCast(len)) * es;
    const so = @as(usize, @intCast(sp)) * es;
    const do_ = @as(usize, @intCast(dp)) * es;
    if (prim or isSubclass(se, de)) {
        if (n == 0) return;
        if (sa == da and do_ > so) {
            std.mem.copyBackwards(u8, db[do_ .. do_ + n], sb[so .. so + n]);
        } else {
            std.mem.copyForwards(u8, db[do_ .. do_ + n], sb[so .. so + n]);
        }
        return;
    }
    // reference arrays with per-element store checks
    const sd = arrayData(Ref, sa);
    const dd = arrayData(Ref, da);
    var i: usize = 0;
    while (i < @as(usize, @intCast(len))) : (i += 1) {
        const v = sd[@as(usize, @intCast(sp)) + i];
        if (v) |val| if (!isSubclass(val.class, de)) {
            return throwKind(.ArrayStore, "arraycopy: element type mismatch");
        };
        dd[@as(usize, @intCast(dp)) + i] = v;
    }
}

// ------------------------------------------------------------------ identity hash
pub fn identityHash(o: *Object) i32 {
    var h = @atomicLoad(u32, &o.hash, .monotonic);
    if (h == 0) {
        // xorshift on a global counter: deterministic per run, never 0
        var x = hash_seed.fetchAdd(0x6D2B79F5, .monotonic);
        x ^= x >> 15;
        x *%= 0x2C1B3C6D;
        x ^= x >> 12;
        x &= 0x7FFFFFFF;
        if (x == 0) x = 1;
        h = @cmpxchgStrong(u32, &o.hash, 0, x, .monotonic, .monotonic) orelse x;
    }
    return @bitCast(h);
}

// ------------------------------------------------------------------ monitors
pub const Monitor = struct {
    mutex: std.Thread.Mutex = .{},
    cond: std.Thread.Condition = .{},
    owner: ?*ThreadState = null,
    count: u32 = 0,
    waiters: u32 = 0,
};

// Monitor side table. An object's monitor index lives in gc_bits >> MON_SHIFT (0: none yet); index i is
// st.mon_dir[i / MON_CHUNK][i % MON_CHUNK]. Directory and chunks are published once and never move, so
// readers need no lock. Indices of dead objects are recycled by the sweep (releaseMonitors) and their
// Monitor structs are reused.
const MON_CHUNK_BITS: u5 = 12;
const MON_CHUNK: u32 = 1 << MON_CHUNK_BITS;
const MON_MAX: u32 = 1 << (32 - @as(u32, MON_SHIFT));
const MON_DIR_LEN: usize = MON_MAX / MON_CHUNK;

inline fn monSlot(idx: u32) *?*Monitor {
    const dir = @atomicLoad(?[*]?[*]?*Monitor, &st.mon_dir, .acquire).?;
    const chunk = @atomicLoad(?[*]?*Monitor, &dir[idx >> MON_CHUNK_BITS], .acquire).?;
    return &chunk[idx & (MON_CHUNK - 1)];
}

/// The monitor of `o` if it has one (never creates it).
pub fn monitorIfAny(o: *Object) ?*Monitor {
    const idx = @atomicLoad(u32, &o.gc_bits, .acquire) >> MON_SHIFT;
    if (idx == 0) return null;
    return monSlot(idx).*;
}

/// Reserve a monitor index with an initialized Monitor behind it (caller publishes it into an object).
fn newMonitorIndex() u32 {
    st.mon_lock.lock();
    defer st.mon_lock.unlock();
    if (st.mon_free.pop()) |idx| return idx;
    const idx = st.mon_next;
    if (idx >= MON_MAX) @panic("monolith: more than 2^30 monitors in use");
    if (st.mon_dir == null) {
        const d = std.heap.page_allocator.alloc(?[*]?*Monitor, MON_DIR_LEN) catch @panic("out of memory");
        @memset(d, null);
        @atomicStore(?[*]?[*]?*Monitor, &st.mon_dir, d.ptr, .release);
    }
    const dir = st.mon_dir.?;
    if (dir[idx >> MON_CHUNK_BITS] == null) {
        const c = gpa.alloc(?*Monitor, MON_CHUNK) catch @panic("out of memory");
        @memset(c, null);
        @atomicStore(?[*]?*Monitor, &dir[idx >> MON_CHUNK_BITS], c.ptr, .release);
    }
    const slot = &dir[idx >> MON_CHUNK_BITS].?[idx & (MON_CHUNK - 1)];
    if (slot.* == null) {
        const m = gpa.create(Monitor) catch @panic("out of memory");
        m.* = .{};
        slot.* = m;
    }
    st.mon_next = idx + 1;
    return idx;
}

/// Recycle the monitor indices of dead objects (collector and lazy slab sweep; the objects are unreachable).
fn releaseMonitors(idxs: []const u32) void {
    if (idxs.len == 0) return;
    st.mon_lock.lock();
    defer st.mon_lock.unlock();
    for (idxs) |idx| monSlot(idx).*.?.* = .{};
    st.mon_free.appendSlice(gpa, idxs) catch @panic("GC: out of memory");
}

fn monitorOf(o: *Object) *Monitor {
    var bits = @atomicLoad(u32, &o.gc_bits, .acquire);
    if (bits >> MON_SHIFT != 0) return monSlot(bits >> MON_SHIFT).*.?;
    const idx = newMonitorIndex();
    while (true) {
        if (bits >> MON_SHIFT != 0) {
            // another thread installed one first: give ours back
            releaseMonitors(&.{idx});
            return monSlot(bits >> MON_SHIFT).*.?;
        }
        bits = @cmpxchgWeak(u32, &o.gc_bits, bits, bits | (idx << MON_SHIFT), .acq_rel, .acquire) orelse
            return monSlot(idx).*.?;
    }
}

pub fn monitorEnter(r: Ref) E!void {
    const o = try nn(r);
    const m = monitorOf(o);
    if (m.owner == cur) {
        m.count += 1;
        return;
    }
    lockSafe(&m.mutex);
    m.owner = cur;
    m.count = 1;
}

pub fn monitorExit(r: Ref) E!void {
    const o = try nn(r);
    const m = monitorOf(o);
    if (m.owner != cur) return throwKind(.IllegalMonitorState, null);
    m.count -= 1;
    if (m.count == 0) {
        m.owner = null;
        m.mutex.unlock();
    }
}

pub fn monitorExitNoThrow(r: Ref) void {
    monitorExit(r) catch {
        cur.exc = null;
    };
}

pub fn monitorWait(r: Ref, millis: i64, nanos: i32) E!void {
    const o = try nn(r);
    const m = monitorOf(o);
    if (m.owner != cur) return throwKind(.IllegalMonitorState, "current thread is not owner");
    if (cur.interrupted.swap(false, .seq_cst)) return throwKind(.Interrupted, null);
    const saved = m.count;
    m.owner = null;
    m.count = 0;
    m.waiters += 1;
    defer m.waiters -= 1;
    cur.sleeper = m;
    enterSafe();
    if (millis == 0 and nanos == 0) {
        m.cond.wait(&m.mutex);
    } else {
        const ns: u64 = @as(u64, @intCast(millis)) * std.time.ns_per_ms + @as(u64, @intCast(nanos));
        m.cond.timedWait(&m.mutex, ns) catch {};
    }
    // We hold m.mutex here; leaving SAFE may park for GC which is fine.
    leaveSafe();
    cur.sleeper = null;
    m.owner = cur;
    m.count = saved;
    if (cur.interrupted.swap(false, .seq_cst)) return throwKind(.Interrupted, null);
}

pub fn monitorNotify(r: Ref, all: bool) E!void {
    const o = try nn(r);
    const m = monitorOf(o);
    if (m.owner != cur) return throwKind(.IllegalMonitorState, "current thread is not owner");
    if (m.waiters == 0) return;
    if (all) m.cond.broadcast() else m.cond.signal();
}

// ------------------------------------------------------------------ strings
const U16Ctx = struct {
    pub fn hash(_: U16Ctx, k: []const u16) u64 {
        return std.hash.Wyhash.hash(0, std.mem.sliceAsBytes(k));
    }
    pub fn eql(_: U16Ctx, a: []const u16, b: []const u16) bool {
        return std.mem.eql(u16, a, b);
    }
};

/// Return the interned java.lang.String for `units` (used by ldc and String.intern()).
pub fn intern(units: []const u16) E!Ref {
    lockSafe(intern_lock);
    if (intern_map.get(units)) |s| {
        intern_lock.unlock();
        return s;
    }
    intern_lock.unlock();
    // interned strings live for the whole process: never inside a scoped arena
    const sd = arenaSuspend();
    const made = hooks.make_string(units);
    arenaResume(sd);
    const s = try made;
    cur.handles[0] = s;
    defer cur.handles[0] = null;
    lockSafe(intern_lock);
    defer intern_lock.unlock();
    if (intern_map.get(units)) |existing| return existing;
    const key = hooks.string_chars(s.?);
    intern_map.put(gpa, key, s) catch @panic("out of memory");
    return s;
}

pub inline fn lit(slot: *Ref, comptime units: []const u16) E!Ref {
    if (@atomicLoad(Ref, slot, .acquire)) |s| return s;
    const s = try intern(units);
    @atomicStore(Ref, slot, s, .release);
    return s;
}

pub fn newStringUtf8(s: []const u8) E!Ref {
    const units = std.unicode.utf8ToUtf16LeAlloc(gpa, s) catch {
        // invalid UTF-8: fall back to Latin-1 interpretation
        const u = gpa.alloc(u16, s.len) catch return throwOOM();
        defer gpa.free(u);
        for (s, 0..) |ch, i| u[i] = ch;
        return hooks.make_string(u);
    };
    defer gpa.free(units);
    return hooks.make_string(units);
}

/// UTF-16 -> UTF-8 (unpaired surrogates become U+FFFD like Java's encoder emits '?').
pub fn stringToUtf8(alloc: std.mem.Allocator, r: Ref) ![]u8 {
    const o = r orelse return alloc.dupe(u8, "null");
    const chars = hooks.string_chars(o);
    var out: std.ArrayList(u8) = .empty;
    errdefer out.deinit(alloc);
    var i: usize = 0;
    while (i < chars.len) : (i += 1) {
        var cp: u21 = chars[i];
        if (cp >= 0xD800 and cp < 0xDC00 and i + 1 < chars.len and chars[i + 1] >= 0xDC00 and chars[i + 1] < 0xE000) {
            cp = 0x10000 + ((@as(u21, chars[i]) - 0xD800) << 10) + (chars[i + 1] - 0xDC00);
            i += 1;
        } else if (cp >= 0xD800 and cp < 0xE000) {
            cp = '?';
        }
        var buf: [4]u8 = undefined;
        const n = std.unicode.utf8Encode(cp, &buf) catch 1;
        try out.appendSlice(alloc, buf[0..n]);
    }
    return out.toOwnedSlice(alloc);
}

// ------------------------------------------------------------------ class mirrors (java.lang.Class)
pub fn classMirror(c: *Class) E!Ref {
    if (@atomicLoad(Ref, &c.mirror, .acquire)) |m| return m;
    // class mirrors are cached in the Class for the whole process: never inside a scoped arena
    const sd = arenaSuspend();
    const made = allocObject(hooks.class_class, hooks.class_class.size);
    arenaResume(sd);
    const m = try made;
    hooks.mirror_set_class(m, c);
    // keep the new mirror rooted: lockSafe may park this thread for a collection
    var fr_refs = [_]Ref{m};
    var fr = Frame{ .refs = &fr_refs, .n = 1, .meth = &rt_meta };
    try pushFrame(&fr);
    defer popFrame(&fr);
    lockSafe(mirror_lock);
    defer mirror_lock.unlock();
    if (c.mirror) |existing| return existing;
    @atomicStore(Ref, &c.mirror, m, .release);
    return m;
}

// ------------------------------------------------------------------ I/O helpers
/// Startup timeline (MONO_GC_VERBOSE): milliseconds since initRuntime.
pub fn startupMark(what: []const u8) void {
    if (!st.gc_verbose) return;
    var buf: [160]u8 = undefined;
    const ms = @divTrunc(std.time.nanoTimestamp() - st.t0_ns, std.time.ns_per_ms);
    writeFd(2, std.fmt.bufPrint(&buf, "[startup +{d} ms: {s}]\n", .{ ms, what }) catch "");
}

pub fn writeFd(fd: i32, bytes: []const u8) void {
    var off: usize = 0;
    while (off < bytes.len) {
        const n = std.posix.write(fd, bytes[off..]) catch return;
        if (n == 0) return;
        off += n;
    }
}

// ------------------------------------------------------------------ threads
pub const default_stack_size: usize = 64 * 1024 * 1024;
const stack_reserve: usize = 256 * 1024;

extern "c" fn pthread_getattr_np(th: std.c.pthread_t, attr: *std.c.pthread_attr_t) c_int;
extern "c" fn pthread_attr_getstack(attr: *const std.c.pthread_attr_t, addr: *?*anyopaque, size: *usize) c_int;
extern "c" fn pthread_attr_destroy(attr: *std.c.pthread_attr_t) c_int;

fn setStackLimit(t: *ThreadState, size: usize) void {
    const base = @frameAddress();
    t.stack_limit = base - size + stack_reserve;
    @atomicStore(usize, &t.poll_limit, t.stack_limit, .monotonic);
    // the collector scans this thread's stack up to its real top: callers of the attaching function (thread
    // entry, JNI host frames) can hold references too
    var attr: std.c.pthread_attr_t = undefined;
    if (pthread_getattr_np(std.c.pthread_self(), &attr) != 0) @panic("monolith GC: pthread_getattr_np failed, cannot find the thread's stack");
    defer _ = pthread_attr_destroy(&attr);
    var addr: ?*anyopaque = null;
    var len: usize = 0;
    if (pthread_attr_getstack(&attr, &addr, &len) != 0 or addr == null) @panic("monolith GC: pthread_attr_getstack failed");
    t.stack_hi = @intFromPtr(addr.?) + len;
    if (t.stack_hi <= base) @panic("monolith GC: stack bounds do not contain the current frame");
}

/// Thread-locals cannot be shared between separately compiled shards: every shard keeps its own `cur` and
/// registers a setter; a thread that attaches updates all of them.
pub fn setCurLocal(t: *ThreadState) void {
    cur = t;
    thread_attached = true;
}

pub fn registerCurSetter(f: *const fn (*ThreadState) void) void {
    // unchecked in ReleaseFast: a build with more shards than slots overwrote the runtime state and crashed at start
    if (st.n_setters >= st.cur_setters.len) @panic("more shards than rt.MAX_SHARDS");
    st.cur_setters[st.n_setters] = f;
    st.n_setters += 1;
}

/// True once this OS thread has a ThreadState (Java threads and attached native threads). JNI's GetEnv and
/// AttachCurrentThread must treat such a thread as attached: attaching it a second time would replace `cur`
/// and break per-thread state such as the class-initialization owner (JNA's JNI_OnLoad did exactly that).
pub threadlocal var thread_attached: bool = false;

pub fn setCur(t: *ThreadState) void {
    cur = t;
    thread_attached = true;
    for (st.cur_setters[0..st.n_setters]) |f| f.?(t);
}

pub fn attachThread(t: *ThreadState, stack_size: usize) void {
    setCur(t);
    setStackLimit(t, stack_size);
    t.state.store(SAFE, .seq_cst);
    registerThread(t);
    _ = thread_count.fetchAdd(1, .seq_cst);
    leaveSafe();
}

pub fn detachThread(t: *ThreadState) void {
    enterSafe();
    unregisterThread(t);
    _ = thread_count.fetchSub(1, .seq_cst);
}

const StartCtx = struct { ts: *ThreadState, stack: usize };

/// Start a native thread for a java.lang.Thread object.
pub fn startThread(jt: Ref, stack_size: i64) E!void {
    const t = gpa.create(ThreadState) catch return throwOOM();
    t.* = .{};
    t.java_thread = jt;
    hooks.thread_eetop_set(jt.?, @bitCast(@as(u64, @intFromPtr(t))));
    const ss: usize = if (stack_size > 0) @max(@as(usize, @intCast(stack_size)), 1024 * 1024) else default_stack_size;
    // The new thread is registered (and therefore its Thread object rooted) before
    // this function returns, by registering it here in SAFE state.
    registerThread(t);
    _ = thread_count.fetchAdd(1, .seq_cst);
    const th = std.Thread.spawn(.{ .stack_size = ss }, threadMainRegistered, .{StartCtx{ .ts = t, .stack = ss }}) catch {
        unregisterThread(t);
        _ = thread_count.fetchSub(1, .seq_cst);
        return throwKind(.OutOfMemory, "unable to create native thread");
    };
    th.detach();
}

fn threadMainRegistered(ctx: StartCtx) void {
    const t = ctx.ts;
    setCur(t);
    setStackLimit(t, ctx.stack);
    leaveSafe();
    const jt = t.java_thread;
    hooks.thread_run(jt) catch {
        const ex = takePending();
        hooks.thread_uncaught(jt, ex) catch {
            _ = takePending();
        };
    };
    detachThread(t);
}

/// Thread.sleep: waits on the thread's park condition so Thread.interrupt() (which unparks)
/// wakes it immediately, like HotSpot.
/// Attach a thread that was created by native code (e.g. an audio or driver thread that
/// invokes a callback): it gets a ThreadState and a java.lang.Thread object.
pub fn attachForeignThread() void {
    const t = gpa.create(ThreadState) catch @panic("out of memory");
    t.* = .{};
    attachThread(t, 8 * 1024 * 1024);
    const jt = hooks.foreign_thread() catch null;
    t.java_thread = jt;
    if (jt) |o| hooks.thread_eetop_set(o, @bitCast(@as(u64, @intFromPtr(t))));
}

pub fn sleepNs(ns: u64) E!void {
    const t = cur;
    if (t.interrupted.swap(false, .seq_cst)) return throwKind(.Interrupted, "sleep interrupted");
    const deadline = std.time.nanoTimestamp() + @as(i128, ns);
    lockSafe(&t.park_mutex);
    enterSafe();
    while (!t.interrupted.load(.seq_cst)) {
        const now = std.time.nanoTimestamp();
        if (now >= deadline) break;
        t.park_cond.timedWait(&t.park_mutex, @intCast(deadline - now)) catch {};
    }
    t.park_mutex.unlock();
    leaveSafe();
    if (t.interrupted.swap(false, .seq_cst)) return throwKind(.Interrupted, "sleep interrupted");
}

// ------------------------------------------------------------------ startup
/// leading `-Dname=value` arguments (like the java launcher): become system properties

pub fn splitJavaOptions(args: [][*:0]u8) [][*:0]u8 {
    var n: usize = 0;
    while (n < args.len and std.mem.startsWith(u8, std.mem.span(args[n]), "-D")) n += 1;
    st.user_props = args[0..n];
    return args[n..];
}

/// First call of the program (before anything touches the runtime state): default-initialize it.
/// getenv through libc: std.os.environ is only set up by the start code of the root shard.
pub fn getenv(key: []const u8) ?[]const u8 {
    var buf: [256]u8 = undefined;
    if (key.len >= buf.len) return null;
    @memcpy(buf[0..key.len], key);
    buf[key.len] = 0;
    const p = std.c.getenv(@ptrCast(&buf)) orelse return null;
    return std.mem.span(p);
}

pub fn environList() []const [*:0]const u8 {
    const env: [*:null]const ?[*:0]const u8 = @ptrCast(std.c.environ);
    var n: usize = 0;
    while (env[n] != null) n += 1;
    return @as([*]const [*:0]const u8, @ptrCast(env))[0..n];
}

/// Raw-bit equality of two primitive results (MONO_PATCH_VERIFY): NaNs with equal payload compare equal.
pub fn patchSame(a: anytype, b: @TypeOf(a)) bool {
    const T = @TypeOf(a);
    return switch (@typeInfo(T)) {
        .float => |f| @as(std.meta.Int(.unsigned, f.bits), @bitCast(a)) == @as(std.meta.Int(.unsigned, f.bits), @bitCast(b)),
        else => a == b,
    };
}

pub noinline fn patchMismatch(name: []const u8) noreturn {
    var buf: [512]u8 = undefined;
    writeFd(2, std.fmt.bufPrint(&buf, "monolith: PATCH MISMATCH in {s}: the hand-written replacement returned a different value than the transpiled original\n", .{name}) catch "monolith: PATCH MISMATCH\n");
    std.process.abort();
}

pub fn initState() void {
    st.* = .{};
}

// ------------------------------------------------------------------ sampling profiler (MONO_PROF=<file>)
// SIGPROF every ~1 ms of process CPU time records the interrupted instruction pointer; a helper thread rewrites
// <file> (raw little-endian u64 addresses) every 3 s. Resolve with tools/prof_report.py against the executable.
const PROF_MAX: usize = 4 * 1024 * 1024;
var prof_buf: [*]u64 = undefined;
var prof_n = std.atomic.Value(usize).init(0);

fn profHandler(_: i32, _: *const std.posix.siginfo_t, ctx: ?*anyopaque) callconv(.c) void {
    const uc: *std.posix.ucontext_t = @ptrCast(@alignCast(ctx.?));
    const pc: u64 = switch (@import("builtin").cpu.arch) {
        .x86_64 => @intCast(uc.mcontext.gregs[std.posix.REG.RIP]),
        .aarch64 => @intCast(uc.mcontext.pc),
        else => 0,
    };
    const i = prof_n.fetchAdd(1, .monotonic);
    if (i < PROF_MAX) prof_buf[i] = pc;
}

fn profWriter(path: [:0]const u8) void {
    while (true) {
        std.Thread.sleep(3 * std.time.ns_per_s);
        const n = @min(prof_n.load(.monotonic), PROF_MAX);
        const f = std.fs.cwd().createFile(path, .{}) catch continue;
        defer f.close();
        f.writeAll(std.mem.sliceAsBytes(prof_buf[0..n])) catch {};
    }
}

fn startProfiler() void {
    const path = getenv("MONO_PROF") orelse return;
    const mem = gpa.alloc(u64, PROF_MAX) catch return;
    prof_buf = mem.ptr;
    var sa = std.posix.Sigaction{
        .handler = .{ .sigaction = profHandler },
        .mask = std.posix.sigemptyset(),
        .flags = std.posix.SA.SIGINFO | std.posix.SA.RESTART,
    };
    std.posix.sigaction(std.posix.SIG.PROF, &sa, null);
    const TV = extern struct { sec: isize, usec: isize };
    const it = extern struct { interval: TV, value: TV }{ .interval = .{ .sec = 0, .usec = 1000 }, .value = .{ .sec = 0, .usec = 1000 } };
    _ = std.os.linux.syscall3(.setitimer, 2, @intFromPtr(&it), 0); // ITIMER_PROF
    const p = gpa.dupeZ(u8, path) catch return;
    _ = std.Thread.spawn(.{}, profWriter, .{p}) catch {};
}

/// A positive whole number from the environment; anything else in a set variable stops the program loudly.
fn envNum(name: []const u8) ?usize {
    const v = getenv(name) orelse return null;
    const n = std.fmt.parseInt(usize, v, 10) catch 0;
    if (n == 0) {
        var buf: [160]u8 = undefined;
        writeFd(2, std.fmt.bufPrint(&buf, "monolith: {s} must be a positive whole number, not '{s}'\n", .{ name, v }) catch "monolith: bad GC setting\n");
        std.process.exit(2);
    }
    return n;
}

pub fn initRuntime(h: Hooks) void {
    st.t0_ns = std.time.nanoTimestamp();
    startProfiler();
    hooks.* = h;
    if (envNum("MONO_GC_MIN_MB")) |mb| {
        st.gc_min_threshold = mb << 20;
        st.gc_threshold = mb << 20;
    }
    // percent of the live heap that may be allocated before the next collection
    if (envNum("MONO_GC_FACTOR")) |pct| st.gc_factor_pct = pct;
    if (envNum("MONO_HEAP_MAX_MB")) |mb| st.gc_max_heap = mb << 20;
    if (envNum("MONO_GC_EXPLICIT_GAP_MS")) |ms| st.gc_explicit_gap_ms = @intCast(ms);
    if (envNum("MONO_GC_MAX_GARBAGE_MB")) |mb| st.gc_max_garbage = mb << 20;
    if (envNum("MONO_HEAP_CENSUS")) |n| st.census_every = @intCast(n);
    if (getenv("MONO_GC_VERBOSE")) |_| st.gc_verbose = true;
    if (getenv("MONO_ALLOC_PROFILE")) |_| st.alloc_profile = true;
    startupMark("runtime initialised");
    array_ifaces.* = .{ h.cloneable, h.serializable };
    for (h.all_classes, 0..) |c, i| c.id = @intCast(i);
}

/// Called from the generated `main`. Runs `entry` on a thread with a big stack.
pub fn runMain(entry: *const fn () E!void) u8 {
    const ss = default_stack_size * 4;
    const th = std.Thread.spawn(.{ .stack_size = ss }, mainThread, .{ entry, ss }) catch @panic("cannot start main thread");
    th.join();
    return st.exit_code;
}

/// Non-blocking Android shared-library entry helper (`mono_android_entry`): spawns `mainThread`
/// with full stack capacity and detaches immediately so the Android UI thread never blocks.
pub fn runMainAsync(entry: *const fn () E!void) i32 {
    const ss = default_stack_size * 4;
    const th = std.Thread.spawn(.{ .stack_size = ss }, mainThread, .{ entry, ss }) catch return -1;
    th.detach();
    return 0;
}

/// Construct JVM argv[] for Android `mono_android_entry` from env vars with C argument fallbacks.
pub fn buildAndroidArgv(user_arg: ?[*:0]const u8, gdir_arg: ?[*:0]const u8, adir_arg: ?[*:0]const u8) [][*:0]u8 {
    const u_slice = getenv("MC_USERNAME") orelse (if (user_arg) |p| std.mem.span(p) else "");
    if (u_slice.len == 0) return &.{};
    const g_slice = getenv("MC_GAME_DIR") orelse (if (gdir_arg) |p| std.mem.span(p) else ".");
    const a_slice = getenv("MC_ASSETS_DIR") orelse (if (adir_arg) |p| std.mem.span(p) else "assets");
    const ver_slice = getenv("MC_VERSION") orelse "MONOLITH";
    const idx_slice = getenv("MC_ASSET_INDEX") orelse "";
    const has_idx = idx_slice.len > 0;
    const n_args: usize = if (has_idx) 14 else 12;

    const list = gpa.alloc([*:0]u8, n_args) catch return &.{};
    const user = gpa.dupeZ(u8, u_slice) catch return &.{};
    const gdir = gpa.dupeZ(u8, g_slice) catch return &.{};
    const adir = gpa.dupeZ(u8, a_slice) catch return &.{};
    const ver = gpa.dupeZ(u8, ver_slice) catch return &.{};

    list[0] = @constCast("--username");
    list[1] = user.ptr;
    list[2] = @constCast("--gameDir");
    list[3] = gdir.ptr;
    list[4] = @constCast("--assetsDir");
    list[5] = adir.ptr;
    list[6] = @constCast("--version");
    list[7] = ver.ptr;
    list[8] = @constCast("--accessToken");
    list[9] = @constCast("0");
    list[10] = @constCast("--userType");
    list[11] = @constCast("legacy");
    if (has_idx) {
        const idx = gpa.dupeZ(u8, idx_slice) catch return list[0..12];
        list[12] = @constCast("--assetIndex");
        list[13] = idx.ptr;
    }
    return list;
}

fn mainThread(entry: *const fn () E!void, ss: usize) void {
    startupMark("engine main thread");
    attachThread(main_ts, ss);
    st.oom_error = hooks.make_exception(.OutOfMemory, "Java heap space") catch null;
    const jt = hooks.thread_start_main() catch {
        reportFatal();
        return;
    };
    main_ts.java_thread = jt;
    hooks.thread_eetop_set(jt.?, @bitCast(@as(u64, @intFromPtr(main_ts))));
    entry() catch {
        const ex = takePending();
        hooks.thread_uncaught(jt, ex) catch reportFatal();
        st.exit_code = 1;
    };
    // JVM semantics: wait for all non-daemon threads, then run shutdown.
    hooks.shutdown() catch reportFatal();
    detachThread(main_ts);
}

fn reportFatal() void {
    writeFd(2, "monolith: fatal: exception during runtime startup/shutdown\n");
    st.exit_code = 1;
}

/// Process termination. On Android the engine lives inside the app process next to HWUI's
/// RenderThread and the GPU driver. libc `exit()` runs their static destructors while those
/// threads still run, which aborts with "pthread_mutex_destroy called on a destroyed mutex".
/// `exit_group` ends the process without running any atexit/destructor chain.
pub fn hardExit(code: u8) noreturn {
    if (builtin.abi.isAndroid()) std.os.linux.exit_group(code);
    std.process.exit(code);
}

pub fn exitProcess(code: i32) noreturn {
    hardExit(@truncate(@as(u32, @bitCast(code))));
}

// ------------------------------------------------------------------ LockSupport park/unpark
pub fn park(absolute: bool, time: i64) void {
    const t = cur;
    if (t.park_state.cmpxchgStrong(1, 0, .seq_cst, .seq_cst) == null) return;
    if (t.interrupted.load(.seq_cst)) return;
    if (absolute and time <= 0) return;
    if (!absolute and time < 0) return;
    var ns: u64 = 0;
    if (absolute or time != 0) {
        if (absolute) {
            const now = std.time.milliTimestamp();
            if (time <= now) return;
            ns = @as(u64, @intCast(time - now)) * std.time.ns_per_ms;
        } else {
            ns = @intCast(time);
            if (ns == 0) return;
        }
    }
    lockSafe(&t.park_mutex);
    defer t.park_mutex.unlock();
    if (t.park_state.cmpxchgStrong(0, 2, .seq_cst, .seq_cst) != null) {
        t.park_state.store(0, .seq_cst);
        return;
    }
    enterSafe();
    if (!absolute and time == 0) {
        t.park_cond.wait(&t.park_mutex);
    } else if (ns > 0) {
        t.park_cond.timedWait(&t.park_mutex, ns) catch {};
    }
    t.park_state.store(0, .seq_cst);
    leaveSafe();
}

pub fn unpark(t: *ThreadState) void {
    if (t.park_state.swap(1, .seq_cst) == 2) {
        lockSafe(&t.park_mutex);
        t.park_cond.signal();
        t.park_mutex.unlock();
    }
}
