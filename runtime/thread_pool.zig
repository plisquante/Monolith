//! Low-power, zero-allocation worker pool for parallel chunk meshing, culling, and asset decode.
//! Workers park cleanly on a condition variable when idle (0% CPU, minimal battery/heat).
//! Callers enqueue function pointers + opaque args into a lock-free/bounded ring or shard queue
//! with an atomic WaitGroup fence so render/tick threads synchronize deterministically.

const std = @import("std");

pub const TaskFn = *const fn (ctx: ?*anyopaque, arg: usize) void;

pub const Task = struct {
    func: TaskFn,
    ctx: ?*anyopaque = null,
    arg: usize = 0,
    wg: ?*WaitGroup = null,
};

pub const WaitGroup = struct {
    pending: std.atomic.Value(usize) = std.atomic.Value(usize).init(0),
    mu: std.Thread.Mutex = .{},
    cv: std.Thread.Condition = .{},

    pub inline fn add(self: *WaitGroup, count: usize) void {
        _ = self.pending.fetchAdd(count, .acq_rel);
    }

    pub inline fn done(self: *WaitGroup) void {
        if (self.pending.fetchSub(1, .acq_rel) == 1) {
            self.mu.lock();
            self.cv.broadcast();
            self.mu.unlock();
        }
    }

    pub fn wait(self: *WaitGroup) void {
        if (self.pending.load(.acquire) == 0) return;
        self.mu.lock();
        defer self.mu.unlock();
        while (self.pending.load(.acquire) > 0) {
            self.cv.wait(&self.mu);
        }
    }
};

pub const QUEUE_CAP: usize = 4096;

pub const WorkerPool = struct {
    const MAX_WORKERS: usize = 16;

    ring: [QUEUE_CAP]Task = undefined,
    head: usize = 0,
    tail: usize = 0,
    count: usize = 0,

    mu: std.Thread.Mutex = .{},
    cv: std.Thread.Condition = .{},
    stopping: bool = false,

    threads: [MAX_WORKERS]?std.Thread = [_]?std.Thread{null} ** MAX_WORKERS,
    num_workers: usize = 0,
    completed_tasks: std.atomic.Value(u64) = std.atomic.Value(u64).init(0),

    pub fn start(self: *WorkerPool, requested_threads: usize) !void {
        const cpus = std.Thread.getCpuCount() catch 2;
        const n = if (requested_threads == 0)
            @max(1, @min(cpus -| 1, 8))
        else
            @max(1, @min(requested_threads, MAX_WORKERS));
        self.stopping = false;
        self.head = 0;
        self.tail = 0;
        self.count = 0;
        self.num_workers = 0;
        for (0..n) |i| {
            self.threads[i] = try std.Thread.spawn(.{ .stack_size = 512 * 1024 }, workerMain, .{self});
            self.num_workers += 1;
        }
    }

    pub fn stop(self: *WorkerPool) void {
        self.mu.lock();
        self.stopping = true;
        self.cv.broadcast();
        self.mu.unlock();
        for (0..self.num_workers) |i| {
            if (self.threads[i]) |th| {
                th.join();
                self.threads[i] = null;
            }
        }
        self.num_workers = 0;
    }

    /// Submit a task. If the bounded queue is momentarily saturated (rare backpressure), runs
    /// synchronously on caller to guarantee forward progress with zero dynamic allocation.
    pub fn submit(self: *WorkerPool, func: TaskFn, ctx: ?*anyopaque, arg: usize, wg: ?*WaitGroup) void {
        if (wg) |w| w.add(1);
        self.mu.lock();
        if (self.stopping or self.num_workers == 0 or self.count == QUEUE_CAP) {
            self.mu.unlock();
            func(ctx, arg);
            _ = self.completed_tasks.fetchAdd(1, .monotonic);
            if (wg) |w| w.done();
            return;
        }
        self.ring[self.tail] = .{ .func = func, .ctx = ctx, .arg = arg, .wg = wg };
        self.tail = (self.tail + 1) & (QUEUE_CAP - 1);
        self.count += 1;
        self.mu.unlock();
        self.cv.signal();
    }

    /// Submit `count` parallel slice tasks [0..count) sharing one context + WaitGroup fence.
    /// Enqueues the entire batch under a single lock acquisition and wakes workers after unlock.
    pub fn dispatchBatch(self: *WorkerPool, func: TaskFn, ctx: ?*anyopaque, count: usize, wg: *WaitGroup) void {
        if (count == 0) return;
        wg.add(@intCast(count));
        var enqueued: usize = 0;
        self.mu.lock();
        if (!self.stopping and self.num_workers > 0) {
            while (enqueued < count and self.count < QUEUE_CAP) : (enqueued += 1) {
                self.ring[self.tail] = .{ .func = func, .ctx = ctx, .arg = enqueued, .wg = wg };
                self.tail = (self.tail + 1) & (QUEUE_CAP - 1);
                self.count += 1;
            }
        }
        self.mu.unlock();
        if (enqueued == 1) {
            self.cv.signal();
        } else if (enqueued > 1) {
            self.cv.broadcast();
        }
        for (enqueued..count) |idx| {
            func(ctx, idx);
            _ = self.completed_tasks.fetchAdd(1, .monotonic);
            wg.done();
        }
    }

    fn workerMain(self: *WorkerPool) void {
        while (true) {
            var t: Task = undefined;
            self.mu.lock();
            while (self.count == 0 and !self.stopping) {
                self.cv.wait(&self.mu);
            }
            if (self.stopping and self.count == 0) {
                self.mu.unlock();
                return;
            }
            t = self.ring[self.head];
            self.head = (self.head + 1) & (QUEUE_CAP - 1);
            self.count -= 1;
            self.mu.unlock();

            t.func(t.ctx, t.arg);
            _ = self.completed_tasks.fetchAdd(1, .monotonic);
            if (t.wg) |w| w.done();
        }
    }
};

fn testSumTask(ctx: ?*anyopaque, idx: usize) void {
    const arr: [*]std.atomic.Value(u32) = @ptrCast(@alignCast(ctx.?));
    _ = arr[idx].store(@intCast(idx * 3 + 1), .release);
}

test "WorkerPool executes batch deterministically with WaitGroup and zero busy-spin" {
    var pool = WorkerPool{};
    try pool.start(2);
    defer pool.stop();

    var results: [64]std.atomic.Value(u32) = undefined;
    for (&results) |*r| r.* = std.atomic.Value(u32).init(0);

    var wg = WaitGroup{};
    pool.dispatchBatch(testSumTask, &results, 64, &wg);
    wg.wait();

    for (&results, 0..) |*r, idx| {
        try std.testing.expectEqual(@as(u32, @intCast(idx * 3 + 1)), r.load(.acquire));
    }
    try std.testing.expectEqual(@as(u64, 64), pool.completed_tasks.load(.monotonic));
}
