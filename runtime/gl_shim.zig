//! Direct Native GL -> libGLESv3.so Dispatch & Android Surface/Input Host Bridge.
//! Used by `rhi_bridge.zig` (`mono.rhi.NativeRhi`) so `--no-lwjgl` OpenGL calls route straight
//! to Android hardware GLESv3 (with GL 3.3 Core -> GLES 3.0 compatibility shims) and GLFW
//! window/input calls bind to TouchCore's `ANativeWindow` + `g_tc_input`.

const std = @import("std");

pub const TcPollSnapshot = extern struct {
    width: i32 = 1280,
    height: i32 = 720,
    key_w: u8 = 0,
    key_a: u8 = 0,
    key_s: u8 = 0,
    key_d: u8 = 0,
    key_jump: u8 = 0,
    key_sneak: u8 = 0,
    key_sprint: u8 = 0,
    key_lmb: u8 = 0,
    key_rmb: u8 = 0,
    key_esc: u8 = 0,
    key_e: u8 = 0,
    key_q: u8 = 0,
    key_f: u8 = 0,
    key_t: u8 = 0,
    key_f3: u8 = 0,
    key_f5: u8 = 0,
    key_tab: u8 = 0,
    third_person: u8 = 0,
    cursor_visible: u8 = 0,
    _pad: [2]u8 = .{ 0, 0 },
    hotbar_slot: i32 = 0,
    look_dx: f32 = 0,
    look_dy: f32 = 0,
    cursor_x: f32 = 0,
    cursor_y: f32 = 0,
};

const HostFns = struct {
    loaded: bool = false,
    lib: ?std.DynLib = null,
    has_window: ?*const fn () callconv(.c) i32 = null,
    get_size: ?*const fn (*i32, *i32) callconv(.c) void = null,
    init_egl: ?*const fn (*i32, *i32) callconv(.c) i32 = null,
    swap_buffers: ?*const fn () callconv(.c) void = null,
    poll_snapshot: ?*const fn (*TcPollSnapshot) callconv(.c) void = null,
    poll_text: ?*const fn ([*]i32, i32) callconv(.c) i32 = null,
    set_grab: ?*const fn (i32) callconv(.c) void = null,
};

var host_lock: std.Thread.Mutex = .{};
var host: HostFns = .{};

fn ensureHost() *const HostFns {
    host_lock.lock();
    defer host_lock.unlock();
    if (!host.loaded) {
        host.loaded = true;
        host.lib = std.DynLib.open("libtouchcore_mono.so") catch null;
        if (host.lib) |*lib| {
            host.has_window = lib.lookup(@TypeOf(host.has_window.?), "tc_android_has_window");
            host.get_size = lib.lookup(@TypeOf(host.get_size.?), "tc_android_get_size");
            host.init_egl = lib.lookup(@TypeOf(host.init_egl.?), "tc_android_init_egl");
            host.swap_buffers = lib.lookup(@TypeOf(host.swap_buffers.?), "tc_android_swap_buffers");
            host.poll_snapshot = lib.lookup(@TypeOf(host.poll_snapshot.?), "tc_android_poll_snapshot");
            host.poll_text = lib.lookup(@TypeOf(host.poll_text.?), "tc_android_poll_text");
            host.set_grab = lib.lookup(@TypeOf(host.set_grab.?), "tc_android_set_grab");
        }
    }
    return &host;
}

pub fn hasOsSurface() bool {
    const h = ensureHost();
    if (h.has_window) |f| return f() != 0;
    return false;
}

pub fn attachOsSurface(out_w: *i32, out_h: *i32) bool {
    const h = ensureHost();
    if (h.init_egl) |f| return f(out_w, out_h) != 0;
    return false;
}

pub fn swapOsSurface() void {
    const h = ensureHost();
    if (h.swap_buffers) |f| f();
}

pub fn osSurfaceSize() struct { w: i32, h: i32 } {
    var w: i32 = 1280;
    var h_px: i32 = 720;
    const h = ensureHost();
    if (h.get_size) |f| f(&w, &h_px);
    return .{ .w = w, .h = h_px };
}

/// Drains typed characters and editing keys from the host: out holds (kind, a, b) triples. Returns the triple count.
pub fn pollText(out: []i32) i32 {
    const h = ensureHost();
    if (h.poll_text) |f| return f(out.ptr, @intCast(out.len / 3));
    return 0;
}

pub fn setMouseGrabbed(grabbed: bool) void {
    const h = ensureHost();
    if (h.set_grab) |f| f(if (grabbed) 1 else 0);
}

pub fn pollSnapshot() TcPollSnapshot {
    var snap = TcPollSnapshot{};
    const h = ensureHost();
    if (h.poll_snapshot) |f| f(&snap);
    return snap;
}

const GlProcs = struct {
    loaded: bool = false,
    is_gles: bool = false,
    lib: ?std.DynLib = null,

    glViewport: ?*const fn (i32, i32, i32, i32) callconv(.c) void = null,
    glClearColor: ?*const fn (f32, f32, f32, f32) callconv(.c) void = null,
    glClear: ?*const fn (u32) callconv(.c) void = null,
    glClearDepth: ?*const fn (f64) callconv(.c) void = null,
    glClearDepthf: ?*const fn (f32) callconv(.c) void = null,
    glDepthRange: ?*const fn (f64, f64) callconv(.c) void = null,
    glDepthRangef: ?*const fn (f32, f32) callconv(.c) void = null,
    glEnable: ?*const fn (u32) callconv(.c) void = null,
    glDisable: ?*const fn (u32) callconv(.c) void = null,
    glBlendFunc: ?*const fn (u32, u32) callconv(.c) void = null,
    glDepthFunc: ?*const fn (u32) callconv(.c) void = null,
    glDepthMask: ?*const fn (u8) callconv(.c) void = null,
    glColorMask: ?*const fn (u8, u8, u8, u8) callconv(.c) void = null,
    glCullFace: ?*const fn (u32) callconv(.c) void = null,
    glFrontFace: ?*const fn (u32) callconv(.c) void = null,
    glScissor: ?*const fn (i32, i32, i32, i32) callconv(.c) void = null,
    glGetError: ?*const fn () callconv(.c) u32 = null,
    glGetIntegerv: ?*const fn (u32, *i32) callconv(.c) void = null,
    glGetString: ?*const fn (u32) callconv(.c) ?[*:0]const u8 = null,

    glGenBuffers: ?*const fn (i32, *u32) callconv(.c) void = null,
    glDeleteBuffers: ?*const fn (i32, *const u32) callconv(.c) void = null,
    glBindBuffer: ?*const fn (u32, u32) callconv(.c) void = null,
    glBufferData: ?*const fn (u32, isize, ?*const anyopaque, u32) callconv(.c) void = null,
    glBufferSubData: ?*const fn (u32, isize, isize, ?*const anyopaque) callconv(.c) void = null,

    glGenVertexArrays: ?*const fn (i32, *u32) callconv(.c) void = null,
    glDeleteVertexArrays: ?*const fn (i32, *const u32) callconv(.c) void = null,
    glBindVertexArray: ?*const fn (u32) callconv(.c) void = null,
    glEnableVertexAttribArray: ?*const fn (u32) callconv(.c) void = null,
    glDisableVertexAttribArray: ?*const fn (u32) callconv(.c) void = null,
    glVertexAttribPointer: ?*const fn (u32, i32, u32, u8, i32, ?*const anyopaque) callconv(.c) void = null,
    glVertexAttribIPointer: ?*const fn (u32, i32, u32, i32, ?*const anyopaque) callconv(.c) void = null,

    glCreateShader: ?*const fn (u32) callconv(.c) u32 = null,
    glDeleteShader: ?*const fn (u32) callconv(.c) void = null,
    glShaderSource: ?*const fn (u32, i32, [*]const [*]const u8, ?[*]const i32) callconv(.c) void = null,
    glCompileShader: ?*const fn (u32) callconv(.c) void = null,
    glGetShaderiv: ?*const fn (u32, u32, *i32) callconv(.c) void = null,

    glCreateProgram: ?*const fn () callconv(.c) u32 = null,
    glDeleteProgram: ?*const fn (u32) callconv(.c) void = null,
    glAttachShader: ?*const fn (u32, u32) callconv(.c) void = null,
    glLinkProgram: ?*const fn (u32) callconv(.c) void = null,
    glUseProgram: ?*const fn (u32) callconv(.c) void = null,
    glGetProgramiv: ?*const fn (u32, u32, *i32) callconv(.c) void = null,
    glGetUniformLocation: ?*const fn (u32, [*:0]const u8) callconv(.c) i32 = null,
    glUniform1i: ?*const fn (i32, i32) callconv(.c) void = null,
    glUniform1f: ?*const fn (i32, f32) callconv(.c) void = null,
    glUniform2f: ?*const fn (i32, f32, f32) callconv(.c) void = null,
    glUniform3f: ?*const fn (i32, f32, f32, f32) callconv(.c) void = null,
    glUniform4f: ?*const fn (i32, f32, f32, f32, f32) callconv(.c) void = null,
    glUniformMatrix4fv: ?*const fn (i32, i32, u8, [*]const f32) callconv(.c) void = null,

    glGenTextures: ?*const fn (i32, *u32) callconv(.c) void = null,
    glDeleteTextures: ?*const fn (i32, *const u32) callconv(.c) void = null,
    glActiveTexture: ?*const fn (u32) callconv(.c) void = null,
    glBindTexture: ?*const fn (u32, u32) callconv(.c) void = null,
    glTexParameteri: ?*const fn (u32, u32, i32) callconv(.c) void = null,
    glTexImage2D: ?*const fn (u32, i32, i32, i32, i32, i32, u32, u32, ?*const anyopaque) callconv(.c) void = null,
    glTexSubImage2D: ?*const fn (u32, i32, i32, i32, i32, i32, u32, u32, ?*const anyopaque) callconv(.c) void = null,

    glGenFramebuffers: ?*const fn (i32, *u32) callconv(.c) void = null,
    glDeleteFramebuffers: ?*const fn (i32, *const u32) callconv(.c) void = null,
    glBindFramebuffer: ?*const fn (u32, u32) callconv(.c) void = null,
    glFramebufferTexture2D: ?*const fn (u32, u32, u32, u32, i32) callconv(.c) void = null,
    glCheckFramebufferStatus: ?*const fn (u32) callconv(.c) u32 = null,
    glBlitFramebuffer: ?*const fn (i32, i32, i32, i32, i32, i32, i32, i32, u32, u32) callconv(.c) void = null,

    glDrawArrays: ?*const fn (u32, i32, i32) callconv(.c) void = null,
    glDrawArraysInstanced: ?*const fn (u32, i32, i32, i32) callconv(.c) void = null,
    glDrawElements: ?*const fn (u32, i32, u32, ?*const anyopaque) callconv(.c) void = null,
    glDrawElementsInstanced: ?*const fn (u32, i32, u32, ?*const anyopaque, i32) callconv(.c) void = null,
    glDrawElementsInstancedBaseVertex: ?*const fn (u32, i32, u32, ?*const anyopaque, i32, i32) callconv(.c) void = null,

    glBindAttribLocation: ?*const fn (u32, u32, [*:0]const u8) callconv(.c) void = null,
    glBindBufferRange: ?*const fn (u32, u32, u32, isize, isize) callconv(.c) void = null,
    glCopyBufferSubData: ?*const fn (u32, u32, isize, isize, isize) callconv(.c) void = null,
    glMapBufferRange: ?*const fn (u32, isize, isize, u32) callconv(.c) ?*anyopaque = null,
    glUnmapBuffer: ?*const fn (u32) callconv(.c) u8 = null,
    glFlushMappedBufferRange: ?*const fn (u32, isize, isize) callconv(.c) void = null,

    glGenSamplers: ?*const fn (i32, *u32) callconv(.c) void = null,
    glDeleteSamplers: ?*const fn (i32, *const u32) callconv(.c) void = null,
    glBindSampler: ?*const fn (u32, u32) callconv(.c) void = null,
    glSamplerParameteri: ?*const fn (u32, u32, i32) callconv(.c) void = null,
    glSamplerParameterf: ?*const fn (u32, u32, f32) callconv(.c) void = null,

    glBlendEquationSeparate: ?*const fn (u32, u32) callconv(.c) void = null,
    glBlendFuncSeparate: ?*const fn (u32, u32, u32, u32) callconv(.c) void = null,
    glClearBufferfv: ?*const fn (u32, i32, [*]const f32) callconv(.c) void = null,
    glColorMaski: ?*const fn (u32, u8, u8, u8, u8) callconv(.c) void = null,
    glEnablei: ?*const fn (u32, u32) callconv(.c) void = null,
    glDisablei: ?*const fn (u32, u32) callconv(.c) void = null,
    glDrawBuffers: ?*const fn (i32, [*]const u32) callconv(.c) void = null,
    glReadBuffer: ?*const fn (u32) callconv(.c) void = null,
    glFinish: ?*const fn () callconv(.c) void = null,
    glGetFloatv: ?*const fn (u32, *f32) callconv(.c) void = null,
    glGetInteger64v: ?*const fn (u32, *i64) callconv(.c) void = null,
    glGetShaderInfoLog: ?*const fn (u32, i32, ?*i32, [*]u8) callconv(.c) void = null,
    glGetProgramInfoLog: ?*const fn (u32, i32, ?*i32, [*]u8) callconv(.c) void = null,
    glGetUniformBlockIndex: ?*const fn (u32, [*:0]const u8) callconv(.c) u32 = null,
    glUniformBlockBinding: ?*const fn (u32, u32, u32) callconv(.c) void = null,
    glPixelStorei: ?*const fn (u32, i32) callconv(.c) void = null,
    glPolygonOffset: ?*const fn (f32, f32) callconv(.c) void = null,
    glReadPixels: ?*const fn (i32, i32, i32, i32, u32, u32, ?*anyopaque) callconv(.c) void = null,
    glTexBuffer: ?*const fn (u32, u32, u32) callconv(.c) void = null,
    glGetTexLevelParameteriv: ?*const fn (u32, i32, u32, *i32) callconv(.c) void = null,
    glGenQueries: ?*const fn (i32, *u32) callconv(.c) void = null,
    glDeleteQueries: ?*const fn (i32, *const u32) callconv(.c) void = null,
    glQueryCounter: ?*const fn (u32, u32) callconv(.c) void = null,
    glGetQueryObjectuiv: ?*const fn (u32, u32, *u32) callconv(.c) void = null,
    glGetQueryObjectui64v: ?*const fn (u32, u32, *u64) callconv(.c) void = null,
    glVertexAttribDivisor: ?*const fn (u32, u32) callconv(.c) void = null,
    glFenceSync: ?*const fn (u32, u32) callconv(.c) ?*anyopaque = null,
    glDeleteSync: ?*const fn (?*anyopaque) callconv(.c) void = null,
    glClientWaitSync: ?*const fn (?*anyopaque, u32, u64) callconv(.c) u32 = null,
};

var gl_lock: std.Thread.Mutex = .{};
var glp: GlProcs = .{};

pub fn ensureGl() *const GlProcs {
    gl_lock.lock();
    defer gl_lock.unlock();
    if (!glp.loaded) {
        glp.loaded = true;
        glp.lib = std.DynLib.open("libGLESv3.so") catch null;
        if (glp.lib != null) {
            glp.is_gles = true;
        } else {
            glp.lib = std.DynLib.open("libGL.so.1") catch (std.DynLib.open("libOpenGL.so.0") catch null);
        }
        if (glp.lib) |*lib| {
            inline for (@typeInfo(GlProcs).@"struct".fields) |fld| {
                if (comptime !std.mem.eql(u8, fld.name, "loaded") and
                    !std.mem.eql(u8, fld.name, "is_gles") and
                    !std.mem.eql(u8, fld.name, "lib"))
                {
                    const FnPtr = @typeInfo(fld.type).optional.child;
                    @field(glp, fld.name) = lib.lookup(FnPtr, fld.name);
                }
            }
        }
    }
    return &glp;
}

pub fn viewport(x: i32, y: i32, w: i32, h: i32) void {
    if (ensureGl().glViewport) |f| f(x, y, @max(w, 1), @max(h, 1));
}
pub fn clearColor(r: f32, g: f32, b: f32, a: f32) void {
    if (ensureGl().glClearColor) |f| f(r, g, b, a);
}
pub fn clear(mask: u32) void {
    if (ensureGl().glClear) |f| f(mask);
}
pub fn clearDepth(d: f64) void {
    const p = ensureGl();
    if (p.glClearDepthf) |f| f(@floatCast(d)) else if (p.glClearDepth) |f| f(d);
}
pub fn depthRange(n: f64, fval: f64) void {
    const p = ensureGl();
    if (p.glDepthRangef) |f| f(@floatCast(n), @floatCast(fval)) else if (p.glDepthRange) |f| f(n, fval);
}
pub fn enable(cap: u32) void {
    if (ensureGl().glEnable) |f| f(cap);
}
pub fn disable(cap: u32) void {
    if (ensureGl().glDisable) |f| f(cap);
}
pub fn blendFunc(s: u32, d: u32) void {
    if (ensureGl().glBlendFunc) |f| f(s, d);
}
pub fn depthFunc(func: u32) void {
    if (ensureGl().glDepthFunc) |f| f(func);
}
pub fn depthMask(flag: u8) void {
    if (ensureGl().glDepthMask) |f| f(flag);
}
pub fn colorMask(r: u8, g: u8, b: u8, a: u8) void {
    if (ensureGl().glColorMask) |f| f(r, g, b, a);
}
pub fn cullFace(mode: u32) void {
    if (ensureGl().glCullFace) |f| f(mode);
}
pub fn frontFace(mode: u32) void {
    if (ensureGl().glFrontFace) |f| f(mode);
}
pub fn scissor(x: i32, y: i32, w: i32, h: i32) void {
    if (ensureGl().glScissor) |f| f(x, y, w, h);
}
pub fn getError() u32 {
    if (ensureGl().glGetError) |f| return f();
    return 0;
}
pub fn getInteger(pname: u32) i32 {
    var out: i32 = 0;
    if (ensureGl().glGetIntegerv) |f| f(pname, &out);
    return out;
}
pub fn getString(name: u32) []const u8 {
    if (ensureGl().glGetString) |f| {
        if (f(name)) |s| return std.mem.span(s);
    }
    return "";
}

pub fn genBuffer() i32 {
    var id: u32 = 0;
    if (ensureGl().glGenBuffers) |f| f(1, &id);
    return @bitCast(id);
}
pub fn deleteBuffer(id: i32) void {
    const uid: u32 = @bitCast(id);
    if (uid != 0) if (ensureGl().glDeleteBuffers) |f| f(1, &uid);
}
pub fn bindBuffer(target: u32, id: i32) void {
    if (ensureGl().glBindBuffer) |f| f(target, @bitCast(id));
}
pub fn bufferData(target: u32, addr: i64, size: i64, usage: u32) void {
    const ptr: ?*const anyopaque = if (addr == 0) null else @ptrFromInt(@as(usize, @bitCast(addr)));
    if (ensureGl().glBufferData) |f| f(target, @intCast(@max(size, 0)), ptr, usage);
}
pub fn bufferSubData(target: u32, off: i64, addr: i64, size: i64) void {
    const ptr: ?*const anyopaque = if (addr == 0) null else @ptrFromInt(@as(usize, @bitCast(addr)));
    if (ptr != null) if (ensureGl().glBufferSubData) |f| f(target, @intCast(off), @intCast(@max(size, 0)), ptr);
}

pub fn genVertexArray() i32 {
    var id: u32 = 0;
    if (ensureGl().glGenVertexArrays) |f| f(1, &id);
    return @bitCast(id);
}
pub fn deleteVertexArray(id: i32) void {
    const uid: u32 = @bitCast(id);
    if (uid != 0) if (ensureGl().glDeleteVertexArrays) |f| f(1, &uid);
}
pub fn bindVertexArray(id: i32) void {
    if (ensureGl().glBindVertexArray) |f| f(@bitCast(id));
}
pub fn enableVertexAttribArray(idx: u32) void {
    if (ensureGl().glEnableVertexAttribArray) |f| f(idx);
}
pub fn disableVertexAttribArray(idx: u32) void {
    if (ensureGl().glDisableVertexAttribArray) |f| f(idx);
}
pub fn vertexAttribPointer(idx: u32, size: i32, typ: u32, norm: u8, stride: i32, off: i64) void {
    const ptr: ?*const anyopaque = @ptrFromInt(@as(usize, @bitCast(off)));
    if (ensureGl().glVertexAttribPointer) |f| f(idx, size, typ, norm, stride, ptr);
}
pub fn vertexAttribIPointer(idx: u32, size: i32, typ: u32, stride: i32, off: i64) void {
    const ptr: ?*const anyopaque = @ptrFromInt(@as(usize, @bitCast(off)));
    if (ensureGl().glVertexAttribIPointer) |f| f(idx, size, typ, stride, ptr);
}

pub fn createShader(kind: u32) i32 {
    if (ensureGl().glCreateShader) |f| return @bitCast(f(kind));
    return 0;
}
pub fn deleteShader(sh: i32) void {
    if (sh != 0) if (ensureGl().glDeleteShader) |f| f(@bitCast(sh));
}
pub fn shaderSourceAdapted(sh: i32, raw_src: []const u8) void {
    const p = ensureGl();
    const sfn = p.glShaderSource orelse return;
    if (p.is_gles) {
        // Rewrite desktop GLSL `#version 150` / `#version 330 core` / `#version 4xx` to ES 3.00; GLSL ES sources
        // (`#version 300 es`, `310 es`, `320 es`, e.g. SPIRV-Cross output) are handed to the driver as they are
        const first_nl = std.mem.indexOfScalar(u8, raw_src, '\n') orelse raw_src.len;
        const is_es = std.mem.startsWith(u8, raw_src, "#version") and std.mem.indexOf(u8, raw_src[0..first_nl], " es") != null;
        if (!is_es) {
            const body_start = if (std.mem.startsWith(u8, raw_src, "#version")) blk: {
                const nl = std.mem.indexOfScalar(u8, raw_src, '\n') orelse raw_src.len;
                break :blk @min(nl + 1, raw_src.len);
            } else 0;
            const hdr = "#version 300 es\nprecision highp float;\nprecision highp int;\n";
            const body = raw_src[body_start..];
            const ptrs = [2][*]const u8{ hdr.ptr, body.ptr };
            const lens = [2]i32{ @intCast(hdr.len), @intCast(body.len) };
            sfn(@bitCast(sh), 2, &ptrs, &lens);
            return;
        }
    }
    const ptrs = [1][*]const u8{raw_src.ptr};
    const lens = [1]i32{@intCast(raw_src.len)};
    sfn(@bitCast(sh), 1, &ptrs, &lens);
}
pub fn compileShader(sh: i32) void {
    if (ensureGl().glCompileShader) |f| f(@bitCast(sh));
}
pub fn getShaderi(sh: i32, pname: u32) i32 {
    var v: i32 = 0;
    if (ensureGl().glGetShaderiv) |f| f(@bitCast(sh), pname, &v);
    return v;
}

pub fn createProgram() i32 {
    if (ensureGl().glCreateProgram) |f| return @bitCast(f());
    return 0;
}
pub fn deleteProgram(prog: i32) void {
    if (prog != 0) if (ensureGl().glDeleteProgram) |f| f(@bitCast(prog));
}
pub fn attachShader(prog: i32, sh: i32) void {
    if (ensureGl().glAttachShader) |f| f(@bitCast(prog), @bitCast(sh));
}
pub fn linkProgram(prog: i32) void {
    if (ensureGl().glLinkProgram) |f| f(@bitCast(prog));
}
pub fn useProgram(prog: i32) void {
    if (ensureGl().glUseProgram) |f| f(@bitCast(prog));
}
pub fn getProgrami(prog: i32, pname: u32) i32 {
    var v: i32 = 0;
    if (ensureGl().glGetProgramiv) |f| f(@bitCast(prog), pname, &v);
    return v;
}
pub fn getUniformLocation(prog: i32, name_z: [*:0]const u8) i32 {
    if (ensureGl().glGetUniformLocation) |f| return f(@bitCast(prog), name_z);
    return -1;
}
pub fn uniform1i(loc: i32, v0: i32) void {
    if (ensureGl().glUniform1i) |f| f(loc, v0);
}
pub fn uniform1f(loc: i32, v0: f32) void {
    if (ensureGl().glUniform1f) |f| f(loc, v0);
}
pub fn uniform2f(loc: i32, v0: f32, v1: f32) void {
    if (ensureGl().glUniform2f) |f| f(loc, v0, v1);
}
pub fn uniform3f(loc: i32, v0: f32, v1: f32, v2: f32) void {
    if (ensureGl().glUniform3f) |f| f(loc, v0, v1, v2);
}
pub fn uniform4f(loc: i32, v0: f32, v1: f32, v2: f32, v3: f32) void {
    if (ensureGl().glUniform4f) |f| f(loc, v0, v1, v2, v3);
}
pub fn uniformMatrix4fv(loc: i32, count: i32, transpose: u8, ptr_addr: i64) void {
    if (ptr_addr == 0) return;
    const p: [*]const f32 = @ptrFromInt(@as(usize, @bitCast(ptr_addr)));
    if (ensureGl().glUniformMatrix4fv) |f| f(loc, count, transpose, p);
}

pub fn genTexture() i32 {
    var id: u32 = 0;
    if (ensureGl().glGenTextures) |f| f(1, &id);
    return @bitCast(id);
}
pub fn deleteTexture(id: i32) void {
    const uid: u32 = @bitCast(id);
    if (uid != 0) if (ensureGl().glDeleteTextures) |f| f(1, &uid);
}
pub fn activeTexture(tex: u32) void {
    if (ensureGl().glActiveTexture) |f| f(tex);
}
pub fn bindTexture(target: u32, id: i32) void {
    if (ensureGl().glBindTexture) |f| f(target, @bitCast(id));
}
pub fn texParameteri(target: u32, pname: u32, param: i32) void {
    if (ensureGl().glTexParameteri) |f| f(target, pname, param);
}
pub fn texImage2D(target: u32, level: i32, internal_fmt: i32, w: i32, h: i32, border: i32, fmt: u32, typ: u32, addr: i64) void {
    const ptr: ?*const anyopaque = if (addr == 0) null else @ptrFromInt(@as(usize, @bitCast(addr)));
    if (ensureGl().glTexImage2D) |f| f(target, level, internal_fmt, w, h, border, fmt, typ, ptr);
}
pub fn texSubImage2D(target: u32, level: i32, xoff: i32, yoff: i32, w: i32, h: i32, fmt: u32, typ: u32, addr: i64) void {
    const ptr: ?*const anyopaque = if (addr == 0) null else @ptrFromInt(@as(usize, @bitCast(addr)));
    if (ensureGl().glTexSubImage2D) |f| f(target, level, xoff, yoff, w, h, fmt, typ, ptr);
}

pub fn genFramebuffer() i32 {
    var id: u32 = 0;
    if (ensureGl().glGenFramebuffers) |f| f(1, &id);
    return @bitCast(id);
}
pub fn deleteFramebuffer(id: i32) void {
    const uid: u32 = @bitCast(id);
    if (uid != 0) if (ensureGl().glDeleteFramebuffers) |f| f(1, &uid);
}
pub fn bindFramebuffer(target: u32, id: i32) void {
    if (ensureGl().glBindFramebuffer) |f| f(target, @bitCast(id));
}
pub fn framebufferTexture2D(target: u32, att: u32, tex_target: u32, tex: i32, level: i32) void {
    if (ensureGl().glFramebufferTexture2D) |f| f(target, att, tex_target, @bitCast(tex), level);
}
pub fn checkFramebufferStatus(target: u32) i32 {
    if (ensureGl().glCheckFramebufferStatus) |f| return @bitCast(f(target));
    return 0;
}

pub fn drawArrays(mode: u32, first: i32, count: i32) void {
    if (ensureGl().glDrawArrays) |f| f(mode, first, count);
}
pub fn drawElements(mode: u32, count: i32, typ: u32, off: i64) void {
    const ptr: ?*const anyopaque = @ptrFromInt(@as(usize, @bitCast(off)));
    if (ensureGl().glDrawElements) |f| f(mode, count, typ, ptr);
}
pub fn drawElementsInstanced(mode: u32, count: i32, typ: u32, off: i64, inst_count: i32) void {
    const ptr: ?*const anyopaque = @ptrFromInt(@as(usize, @bitCast(off)));
    if (ensureGl().glDrawElementsInstanced) |f| f(mode, count, typ, ptr, inst_count);
}
pub fn drawArraysInstanced(mode: u32, first: i32, count: i32, inst_count: i32) void {
    const p = ensureGl();
    if (p.glDrawArraysInstanced) |f| {
        f(mode, first, count, inst_count);
    } else if (p.glDrawArrays) |fa| {
        fa(mode, first, count);
    }
}
pub fn drawElementsInstancedBaseVertex(mode: u32, count: i32, typ: u32, off: i64, inst_count: i32, base_vertex: i32) void {
    const ptr: ?*const anyopaque = @ptrFromInt(@as(usize, @bitCast(off)));
    const p = ensureGl();
    if (p.glDrawElementsInstancedBaseVertex) |f| {
        f(mode, count, typ, ptr, inst_count, base_vertex);
    } else if (base_vertex == 0) {
        if (p.glDrawElementsInstanced) |fe| fe(mode, count, typ, ptr, inst_count);
    }
}
pub fn blitFramebuffer(sx0: i32, sy0: i32, sx1: i32, sy1: i32, dx0: i32, dy0: i32, dx1: i32, dy1: i32, mask: u32, filter: u32) void {
    if (ensureGl().glBlitFramebuffer) |f| f(sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, mask, filter);
}
pub fn bindAttribLocation(prog: i32, idx: i32, name_z: [*:0]const u8) void {
    if (ensureGl().glBindAttribLocation) |f| f(@bitCast(prog), @bitCast(idx), name_z);
}
pub fn bindBufferRange(target: u32, idx: i32, buf: i32, off: i64, sz: i64) void {
    if (ensureGl().glBindBufferRange) |f| f(target, @bitCast(idx), @bitCast(buf), @intCast(off), @intCast(sz));
}
pub fn copyBufferSubData(rtgt: u32, wtgt: u32, roff: i64, woff: i64, sz: i64) void {
    if (ensureGl().glCopyBufferSubData) |f| f(rtgt, wtgt, @intCast(roff), @intCast(woff), @intCast(sz));
}
pub fn mapBufferRange(target: u32, off: i64, len: i64, access: u32) i64 {
    if (ensureGl().glMapBufferRange) |f| {
        const ptr = f(target, @intCast(off), @intCast(@max(len, 0)), access);
        if (ptr) |p| return @bitCast(@as(u64, @intFromPtr(p)));
    }
    return 0;
}
pub fn unmapBuffer(target: u32) bool {
    if (ensureGl().glUnmapBuffer) |f| return f(target) != 0;
    return true;
}
pub fn flushMappedBufferRange(target: u32, off: i64, len: i64) void {
    if (ensureGl().glFlushMappedBufferRange) |f| f(target, @intCast(off), @intCast(@max(len, 0)));
}
pub fn genSampler() i32 {
    var id: u32 = 0;
    if (ensureGl().glGenSamplers) |f| f(1, &id);
    return @bitCast(id);
}
pub fn deleteSampler(id: i32) void {
    const uid: u32 = @bitCast(id);
    if (uid != 0) if (ensureGl().glDeleteSamplers) |f| f(1, &uid);
}
pub fn bindSampler(unit: i32, sampler: i32) void {
    if (ensureGl().glBindSampler) |f| f(@bitCast(unit), @bitCast(sampler));
}
pub fn samplerParameteri(sampler: i32, pname: u32, param: i32) void {
    if (ensureGl().glSamplerParameteri) |f| f(@bitCast(sampler), pname, param);
}
pub fn samplerParameterf(sampler: i32, pname: u32, param: f32) void {
    if (ensureGl().glSamplerParameterf) |f| f(@bitCast(sampler), pname, param);
}
pub fn blendEquationSeparate(rgb: u32, alpha: u32) void {
    if (ensureGl().glBlendEquationSeparate) |f| f(rgb, alpha);
}
pub fn blendFuncSeparate(sr: u32, dr: u32, sa: u32, da: u32) void {
    if (ensureGl().glBlendFuncSeparate) |f| f(sr, dr, sa, da);
}
pub fn clearBufferfv(buf: u32, drawbuf: i32, vals: [*]const f32) void {
    if (ensureGl().glClearBufferfv) |f| f(buf, drawbuf, vals);
}
pub fn colorMaski(idx: i32, r: u8, g: u8, b: u8, a: u8) void {
    const p = ensureGl();
    if (p.glColorMaski) |f| {
        f(@bitCast(idx), r, g, b, a);
    } else if (idx == 0) {
        if (p.glColorMask) |fc| fc(r, g, b, a);
    }
}
pub fn enablei(cap: u32, idx: i32) void {
    const p = ensureGl();
    if (p.glEnablei) |f| {
        f(cap, @bitCast(idx));
    } else if (idx == 0) {
        if (p.glEnable) |fe| fe(cap);
    }
}
pub fn disablei(cap: u32, idx: i32) void {
    const p = ensureGl();
    if (p.glDisablei) |f| {
        f(cap, @bitCast(idx));
    } else if (idx == 0) {
        if (p.glDisable) |fd| fd(cap);
    }
}
pub fn drawBuffers(n: i32, bufs: [*]const u32) void {
    if (ensureGl().glDrawBuffers) |f| f(n, bufs);
}
pub fn readBuffer(src: u32) void {
    if (ensureGl().glReadBuffer) |f| f(src);
}
pub fn finish() void {
    if (ensureGl().glFinish) |f| f();
}
pub fn getFloat(pname: u32) f32 {
    var v: f32 = 0;
    if (ensureGl().glGetFloatv) |f| f(pname, &v);
    return v;
}
pub fn getInteger64(pname: u32) i64 {
    var v: i64 = 0;
    if (ensureGl().glGetInteger64v) |f| f(pname, &v);
    return v;
}
pub fn getShaderInfoLog(sh: i32, out_buf: []u8) []u8 {
    var len: i32 = 0;
    if (ensureGl().glGetShaderInfoLog) |f| {
        f(@bitCast(sh), @intCast(out_buf.len), &len, out_buf.ptr);
        const n: usize = @intCast(@max(0, @min(len, @as(i32, @intCast(out_buf.len)))));
        return out_buf[0..n];
    }
    return out_buf[0..0];
}
pub fn getProgramInfoLog(prog: i32, out_buf: []u8) []u8 {
    var len: i32 = 0;
    if (ensureGl().glGetProgramInfoLog) |f| {
        f(@bitCast(prog), @intCast(out_buf.len), &len, out_buf.ptr);
        const n: usize = @intCast(@max(0, @min(len, @as(i32, @intCast(out_buf.len)))));
        return out_buf[0..n];
    }
    return out_buf[0..0];
}
pub fn getUniformBlockIndex(prog: i32, name_z: [*:0]const u8) i32 {
    if (ensureGl().glGetUniformBlockIndex) |f| return @bitCast(f(@bitCast(prog), name_z));
    return -1;
}
pub fn uniformBlockBinding(prog: i32, idx: i32, binding: i32) void {
    if (ensureGl().glUniformBlockBinding) |f| f(@bitCast(prog), @bitCast(idx), @bitCast(binding));
}
pub fn pixelStorei(pname: u32, param: i32) void {
    if (ensureGl().glPixelStorei) |f| f(pname, param);
}
pub fn polygonOffset(factor: f32, units: f32) void {
    if (ensureGl().glPolygonOffset) |f| f(factor, units);
}
pub fn readPixels(x: i32, y: i32, w: i32, h: i32, fmt: u32, typ: u32, addr: i64) void {
    const ptr: ?*anyopaque = if (addr == 0) null else @ptrFromInt(@as(usize, @bitCast(addr)));
    if (ensureGl().glReadPixels) |f| f(x, y, w, h, fmt, typ, ptr);
}
pub fn texBuffer(target: u32, ifmt: u32, buf: i32) void {
    if (ensureGl().glTexBuffer) |f| f(target, ifmt, @bitCast(buf));
}
pub fn getTexLevelParameteri(target: u32, level: i32, pname: u32) i32 {
    var v: i32 = 0;
    if (ensureGl().glGetTexLevelParameteriv) |f| f(target, level, pname, &v);
    return v;
}
pub fn genQuery() i32 {
    var id: u32 = 0;
    if (ensureGl().glGenQueries) |f| f(1, &id);
    return @bitCast(id);
}
pub fn deleteQuery(id: i32) void {
    const uid: u32 = @bitCast(id);
    if (uid != 0) if (ensureGl().glDeleteQueries) |f| f(1, &uid);
}
pub fn queryCounter(id: i32, target: u32) void {
    if (ensureGl().glQueryCounter) |f| f(@bitCast(id), target);
}
pub fn getQueryObjecti(id: i32, pname: u32) i32 {
    var v: u32 = 0;
    if (ensureGl().glGetQueryObjectuiv) |f| f(@bitCast(id), pname, &v);
    return @bitCast(v);
}
pub fn getQueryObjectui64(id: i32, pname: u32) i64 {
    var v: u64 = 0;
    if (ensureGl().glGetQueryObjectui64v) |f| f(@bitCast(id), pname, &v);
    return @bitCast(v);
}
pub fn vertexAttribDivisor(index: u32, divisor: u32) void {
    if (ensureGl().glVertexAttribDivisor) |f| f(index, divisor);
}
pub fn fenceSync(condition: u32, flags: u32) i64 {
    if (ensureGl().glFenceSync) |f| {
        if (f(condition, flags)) |s| return @intCast(@intFromPtr(s));
    }
    return 1;
}
pub fn deleteSync(sync: i64) void {
    if (sync <= 1) return;
    if (ensureGl().glDeleteSync) |f| f(@ptrFromInt(@as(usize, @bitCast(sync))));
}
pub fn clientWaitSync(sync: i64, flags: u32, timeout_ns: i64) i32 {
    if (sync <= 1) return 0x911A; // GL_ALREADY_SIGNALED
    if (ensureGl().glClientWaitSync) |f| return @bitCast(f(@ptrFromInt(@as(usize, @bitCast(sync))), flags, @bitCast(timeout_ns)));
    return 0x911A;
}
