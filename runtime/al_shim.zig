//! Optional dynamic OpenAL dispatch (`libopenal.so.1` / `libopenal.so`) with safe deterministic fallback
//! so Minecraft `SoundEngine` operates cleanly on Android/headless targets when native OpenAL is absent.

const std = @import("std");

const AlProcs = struct {
    loaded: bool = false,
    lib: ?std.DynLib = null,

    alcOpenDevice: ?*const fn (?[*:0]const u8) callconv(.c) ?*anyopaque = null,
    alcCloseDevice: ?*const fn (?*anyopaque) callconv(.c) u8 = null,
    alcCreateContext: ?*const fn (?*anyopaque, ?[*]const i32) callconv(.c) ?*anyopaque = null,
    alcMakeContextCurrent: ?*const fn (?*anyopaque) callconv(.c) u8 = null,
    alcDestroyContext: ?*const fn (?*anyopaque) callconv(.c) void = null,
    alcGetError: ?*const fn (?*anyopaque) callconv(.c) i32 = null,
    alcGetIntegerv: ?*const fn (?*anyopaque, i32, i32, [*]i32) callconv(.c) void = null,
    alcGetString: ?*const fn (?*anyopaque, i32) callconv(.c) ?[*:0]const u8 = null,
    alcIsExtensionPresent: ?*const fn (?*anyopaque, [*:0]const u8) callconv(.c) u8 = null,

    alGenBuffers: ?*const fn (i32, [*]u32) callconv(.c) void = null,
    alDeleteBuffers: ?*const fn (i32, [*]const u32) callconv(.c) void = null,
    alBufferData: ?*const fn (u32, i32, ?*const anyopaque, i32, i32) callconv(.c) void = null,
    alGenSources: ?*const fn (i32, [*]u32) callconv(.c) void = null,
    alDeleteSources: ?*const fn (i32, [*]const u32) callconv(.c) void = null,
    alEnable: ?*const fn (i32) callconv(.c) void = null,
    alSourcePlay: ?*const fn (u32) callconv(.c) void = null,
    alSourcePause: ?*const fn (u32) callconv(.c) void = null,
    alSourceStop: ?*const fn (u32) callconv(.c) void = null,
    alSourceQueueBuffers: ?*const fn (u32, i32, [*]const u32) callconv(.c) void = null,
    alSourceUnqueueBuffers: ?*const fn (u32, i32, [*]u32) callconv(.c) void = null,
    alGetSourcei: ?*const fn (u32, i32, *i32) callconv(.c) void = null,
    alSourcei: ?*const fn (u32, i32, i32) callconv(.c) void = null,
    alSourcef: ?*const fn (u32, i32, f32) callconv(.c) void = null,
    alSource3f: ?*const fn (u32, i32, f32, f32, f32) callconv(.c) void = null,
    alSourcefv: ?*const fn (u32, i32, [*]const f32) callconv(.c) void = null,
    alListener3f: ?*const fn (i32, f32, f32, f32) callconv(.c) void = null,
    alListenerfv: ?*const fn (i32, [*]const f32) callconv(.c) void = null,
    alGetError: ?*const fn () callconv(.c) i32 = null,
};

var al_lock: std.Thread.Mutex = .{};
var alp: AlProcs = .{};
var next_id = std.atomic.Value(i32).init(1);

fn ensureAl() *const AlProcs {
    al_lock.lock();
    defer al_lock.unlock();
    if (!alp.loaded) {
        alp.loaded = true;
        alp.lib = std.DynLib.open("libopenal.so.1") catch (std.DynLib.open("libopenal.so") catch null);
        if (alp.lib) |*lib| {
            inline for (@typeInfo(AlProcs).@"struct".fields) |fld| {
                if (comptime !std.mem.eql(u8, fld.name, "loaded") and !std.mem.eql(u8, fld.name, "lib")) {
                    const FnPtr = @typeInfo(fld.type).optional.child;
                    @field(alp, fld.name) = lib.lookup(FnPtr, fld.name);
                }
            }
        }
    }
    return &alp;
}

pub fn hasNativeOpenAL() bool {
    return ensureAl().lib != null;
}

pub fn alcOpenDevice(name_z: ?[*:0]const u8) i64 {
    if (ensureAl().alcOpenDevice) |f| {
        const ptr = f(name_z);
        return if (ptr) |p| @bitCast(@as(u64, @intFromPtr(p))) else 0;
    }
    return 1;
}

pub fn alcCloseDevice(dev: i64) bool {
    if (ensureAl().alcCloseDevice) |f| {
        const p: ?*anyopaque = if (dev == 0) null else @ptrFromInt(@as(usize, @bitCast(dev)));
        return f(p) != 0;
    }
    return true;
}

pub fn alcCreateContext(dev: i64, attr_addr: i64) i64 {
    if (ensureAl().alcCreateContext) |f| {
        const d: ?*anyopaque = if (dev == 0) null else @ptrFromInt(@as(usize, @bitCast(dev)));
        const a: ?[*]const i32 = if (attr_addr == 0) null else @ptrFromInt(@as(usize, @bitCast(attr_addr)));
        const ptr = f(d, a);
        return if (ptr) |p| @bitCast(@as(u64, @intFromPtr(p))) else 0;
    }
    return 1;
}

pub fn alcMakeContextCurrent(ctx: i64) bool {
    if (ensureAl().alcMakeContextCurrent) |f| {
        const c: ?*anyopaque = if (ctx == 0) null else @ptrFromInt(@as(usize, @bitCast(ctx)));
        return f(c) != 0;
    }
    return true;
}

pub fn alcDestroyContext(ctx: i64) void {
    if (ensureAl().alcDestroyContext) |f| {
        const c: ?*anyopaque = if (ctx == 0) null else @ptrFromInt(@as(usize, @bitCast(ctx)));
        f(c);
    }
}

pub fn alcGetInteger(dev: i64, param: i32) i32 {
    var v: i32 = 0;
    if (ensureAl().alcGetIntegerv) |f| {
        const d: ?*anyopaque = if (dev == 0) null else @ptrFromInt(@as(usize, @bitCast(dev)));
        f(d, param, 1, @ptrCast(&v));
        return v;
    }
    // ALC_FREQUENCY=0x1007 -> 44100, ALC_REFRESH=0x1008 -> 60, ALC_MONO_SOURCES=0x1010 -> 255
    return switch (param) {
        0x1007 => 44100,
        0x1008 => 60,
        0x1010 => 255,
        0x1011 => 1,
        else => 1,
    };
}

pub fn alcGetString(dev: i64, param: i32) []const u8 {
    if (ensureAl().alcGetString) |f| {
        const d: ?*anyopaque = if (dev == 0) null else @ptrFromInt(@as(usize, @bitCast(dev)));
        if (f(d, param)) |s| return std.mem.span(s);
    }
    return "J2ZIG OpenAL";
}

pub fn alGenId() i32 {
    if (ensureAl().alGenBuffers) |f| {
        var id: u32 = 0;
        f(1, @ptrCast(&id));
        if (id != 0) return @bitCast(id);
    }
    return next_id.fetchAdd(1, .monotonic);
}

pub fn alGenSourceId() i32 {
    if (ensureAl().alGenSources) |f| {
        var id: u32 = 0;
        f(1, @ptrCast(&id));
        if (id != 0) return @bitCast(id);
    }
    return next_id.fetchAdd(1, .monotonic);
}

pub fn alDeleteBuffer(id: i32) void {
    if (ensureAl().alDeleteBuffers) |f| {
        var uid: u32 = @bitCast(id);
        f(1, @ptrCast(&uid));
    }
}

pub fn alDeleteSource(id: i32) void {
    if (ensureAl().alDeleteSources) |f| {
        var uid: u32 = @bitCast(id);
        f(1, @ptrCast(&uid));
    }
}

pub fn alBufferData(buf: i32, fmt: i32, addr: i64, size: i32, freq: i32) void {
    if (ensureAl().alBufferData) |f| {
        const ptr: ?*const anyopaque = if (addr == 0) null else @ptrFromInt(@as(usize, @bitCast(addr)));
        f(@bitCast(buf), fmt, ptr, size, freq);
    }
}

pub fn alSourcePlay(src: i32) void {
    if (ensureAl().alSourcePlay) |f| f(@bitCast(src));
}
pub fn alSourcePause(src: i32) void {
    if (ensureAl().alSourcePause) |f| f(@bitCast(src));
}
pub fn alSourceStop(src: i32) void {
    if (ensureAl().alSourceStop) |f| f(@bitCast(src));
}

pub fn alSourceQueueBuffer(src: i32, buf: i32) void {
    if (ensureAl().alSourceQueueBuffers) |f| {
        var b: u32 = @bitCast(buf);
        f(@bitCast(src), 1, @ptrCast(&b));
    }
}

pub fn alSourceUnqueueBuffer(src: i32) i32 {
    if (ensureAl().alSourceUnqueueBuffers) |f| {
        var b: u32 = 0;
        f(@bitCast(src), 1, @ptrCast(&b));
        return @bitCast(b);
    }
    return 0;
}

pub fn alGetSourcei(src: i32, param: i32) i32 {
    if (ensureAl().alGetSourcei) |f| {
        var out: i32 = 0;
        f(@bitCast(src), param, &out);
        return out;
    }
    // AL_SOURCE_STATE (0x1010) -> AL_STOPPED (0x1014) so callers finish cleanly
    return if (param == 0x1010) 0x1014 else 0;
}

pub fn alSourcei(src: i32, param: i32, val: i32) void {
    if (ensureAl().alSourcei) |f| f(@bitCast(src), param, val);
}
pub fn alSourcef(src: i32, param: i32, val: f32) void {
    if (ensureAl().alSourcef) |f| f(@bitCast(src), param, val);
}
pub fn alSource3f(src: i32, param: i32, x: f32, y: f32, z: f32) void {
    if (ensureAl().alSource3f) |f| f(@bitCast(src), param, x, y, z);
}
pub fn alListener3f(param: i32, x: f32, y: f32, z: f32) void {
    if (ensureAl().alListener3f) |f| f(param, x, y, z);
}
pub fn alGetError() i32 {
    if (ensureAl().alGetError) |f| return f();
    return 0;
}
