//! OpenGL 3.3 Core Dynamic RHI Backend for legacy integrated GPUs (e.g., Intel HD 5500 / Gen8).
//! Strictly obeys OpenGL 3.3 Core Profile capabilities:
//!   - ZERO requirement on GL 4.3 SSBO or GL 4.3 `glMultiDrawElementsIndirect`.
//!   - GPU instancing data streamed via triple-buffered orphan (`glBufferData(NULL)`) +
//!     `glMapBufferRange(GL_MAP_WRITE_BIT | GL_MAP_UNSYNCHRONIZED_BIT)` -> `GL_TEXTURE_BUFFER` (TBO `samplerBuffer`).
//!   - Dispatches real GL 3.3 Core commands (`glClearColor`, `glClear`, `glViewport`, `glBindBuffer`,
//!     `glBufferData`, `glMapBufferRange`, `glUnmapBuffer`, `glTexBuffer`, `glDrawElementsInstancedBaseVertex`)
//!     whenever an active GL context is bound.

const std = @import("std");
const rhi = @import("rhi.zig");

pub const GLenum = u32;
pub const GLint = i32;
pub const GLuint = u32;
pub const GLsizei = i32;
pub const GLsizeiptr = isize;
pub const GLbitfield = u32;

pub const GL_VERSION: GLenum = 0x1F02;
pub const GL_MAJOR_VERSION: GLenum = 0x821B;
pub const GL_MINOR_VERSION: GLenum = 0x821C;
pub const GL_COLOR_BUFFER_BIT: GLbitfield = 0x00004000;
pub const GL_DEPTH_BUFFER_BIT: GLbitfield = 0x00000100;
pub const GL_ARRAY_BUFFER: GLenum = 0x8892;
pub const GL_ELEMENT_ARRAY_BUFFER: GLenum = 0x8893;
pub const GL_TEXTURE_BUFFER: GLenum = 0x8C2A;
pub const GL_TEXTURE0: GLenum = 0x84C0;
pub const GL_STREAM_DRAW: GLenum = 0x88E0;
pub const GL_STATIC_DRAW: GLenum = 0x88E4;
pub const GL_MAP_WRITE_BIT: GLbitfield = 0x0002;
pub const GL_MAP_UNSYNCHRONIZED_BIT: GLbitfield = 0x0020;
pub const GL_TRIANGLES: GLenum = 0x0004;
pub const GL_UNSIGNED_INT: GLenum = 0x1405;
pub const GL_RGBA32F: GLenum = 0x8814;
pub const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
pub const GL_VERTEX_SHADER: GLenum = 0x8B31;
pub const GL_COMPILE_STATUS: GLenum = 0x8B81;
pub const GL_LINK_STATUS: GLenum = 0x8B82;

pub const Gl33Procs = struct {
    glGetString: ?*const fn (GLenum) callconv(.c) ?[*:0]const u8 = null,
    glGetIntegerv: ?*const fn (GLenum, *GLint) callconv(.c) void = null,
    glViewport: ?*const fn (GLint, GLint, GLsizei, GLsizei) callconv(.c) void = null,
    glClearColor: ?*const fn (f32, f32, f32, f32) callconv(.c) void = null,
    glClearDepth: ?*const fn (f64) callconv(.c) void = null,
    glClearDepthf: ?*const fn (f32) callconv(.c) void = null,
    glClear: ?*const fn (GLbitfield) callconv(.c) void = null,
    glGenBuffers: ?*const fn (GLsizei, [*]GLuint) callconv(.c) void = null,
    glDeleteBuffers: ?*const fn (GLsizei, [*]const GLuint) callconv(.c) void = null,
    glGenTextures: ?*const fn (GLsizei, [*]GLuint) callconv(.c) void = null,
    glDeleteTextures: ?*const fn (GLsizei, [*]const GLuint) callconv(.c) void = null,
    glActiveTexture: ?*const fn (GLenum) callconv(.c) void = null,
    glBindTexture: ?*const fn (GLenum, GLuint) callconv(.c) void = null,
    glGenVertexArrays: ?*const fn (GLsizei, [*]GLuint) callconv(.c) void = null,
    glDeleteVertexArrays: ?*const fn (GLsizei, [*]const GLuint) callconv(.c) void = null,
    glBindVertexArray: ?*const fn (GLuint) callconv(.c) void = null,
    glCreateShader: ?*const fn (GLenum) callconv(.c) GLuint = null,
    glShaderSource: ?*const fn (GLuint, GLsizei, [*]const [*:0]const u8, ?[*]const GLint) callconv(.c) void = null,
    glCompileShader: ?*const fn (GLuint) callconv(.c) void = null,
    glGetShaderiv: ?*const fn (GLuint, GLenum, *GLint) callconv(.c) void = null,
    glDeleteShader: ?*const fn (GLuint) callconv(.c) void = null,
    glCreateProgram: ?*const fn () callconv(.c) GLuint = null,
    glAttachShader: ?*const fn (GLuint, GLuint) callconv(.c) void = null,
    glLinkProgram: ?*const fn (GLuint) callconv(.c) void = null,
    glGetProgramiv: ?*const fn (GLuint, GLenum, *GLint) callconv(.c) void = null,
    glDeleteProgram: ?*const fn (GLuint) callconv(.c) void = null,
    glGetUniformLocation: ?*const fn (GLuint, [*:0]const u8) callconv(.c) GLint = null,
    glUniform1i: ?*const fn (GLint, GLint) callconv(.c) void = null,
    glUseProgram: ?*const fn (GLuint) callconv(.c) void = null,
    glBindBuffer: ?*const fn (GLenum, GLuint) callconv(.c) void = null,
    glBufferData: ?*const fn (GLenum, GLsizeiptr, ?*const anyopaque, GLenum) callconv(.c) void = null,
    glMapBufferRange: ?*const fn (GLenum, isize, isize, GLbitfield) callconv(.c) ?*anyopaque = null,
    glUnmapBuffer: ?*const fn (GLenum) callconv(.c) u8 = null,
    glTexBuffer: ?*const fn (GLenum, GLenum, GLuint) callconv(.c) void = null,
    glDrawElementsInstanced: ?*const fn (GLenum, GLsizei, GLenum, ?*const anyopaque, GLsizei) callconv(.c) void = null,
    glDrawElementsInstancedBaseVertex: ?*const fn (GLenum, GLsizei, GLenum, ?*const anyopaque, GLsizei, GLint) callconv(.c) void = null,
    glDrawElementsInstancedBaseVertexBaseInstance: ?*const fn (GLenum, GLsizei, GLenum, ?*const anyopaque, GLsizei, GLint, GLuint) callconv(.c) void = null,
};

const VS_SRC_330: [*:0]const u8 =
    \\#version 330 core
    \\layout(location = 0) in vec3 aPos;
    \\uniform samplerBuffer uInstances;
    \\uniform int uFirstInstance;
    \\out vec4 vColor;
    \\void main() {
    \\    int base = (gl_InstanceID + uFirstInstance) * 4;
    \\    vec4 r0 = texelFetch(uInstances, base + 0);
    \\    vec4 r1 = texelFetch(uInstances, base + 1);
    \\    vec4 r2 = texelFetch(uInstances, base + 2);
    \\    vec3 wp = vec3(dot(r0.xyz, aPos) + r0.w, dot(r1.xyz, aPos) + r1.w, dot(r2.xyz, aPos) + r2.w);
    \\    gl_Position = vec4(wp * 0.01, 1.0);
    \\    vColor = vec4(1.0);
    \\}
;

const FS_SRC_330: [*:0]const u8 =
    \\#version 330 core
    \\in vec4 vColor;
    \\out vec4 oColor;
    \\void main() { oColor = vColor; }
;

const VS_SRC_ES300: [*:0]const u8 =
    \\#version 300 es
    \\layout(location = 0) in vec3 aPos;
    \\uniform int uFirstInstance;
    \\out vec4 vColor;
    \\void main() {
    \\    gl_Position = vec4(aPos * 0.01 + vec3(float(gl_InstanceID + uFirstInstance) * 0.0001, 0.0, 0.0), 1.0);
    \\    vColor = vec4(1.0);
    \\}
;

const FS_SRC_ES300: [*:0]const u8 =
    \\#version 300 es
    \\precision mediump float;
    \\in vec4 vColor;
    \\out vec4 oColor;
    \\void main() { oColor = vColor; }
;

pub const Gl33Backend = struct {
    const MAX_HANDLES = 512;

    lib_gl: std.DynLib,
    lib_egl: ?std.DynLib = null,
    procs: Gl33Procs = .{},
    major: i32 = 3,
    minor: i32 = 3,
    has_base_instance: bool = false,
    gl_buffers: [MAX_HANDLES]GLuint = [_]GLuint{0} ** MAX_HANDLES,
    gl_tbo_tex: GLuint = 0,
    gl_default_vao: GLuint = 0,
    gl_default_prog: GLuint = 0,
    u_first_inst_loc: GLint = -1,
    tbo_uploads: u64 = 0,
    lowered_instanced_draws: u64 = 0,

    fn resolveProc(
        glx_gp: ?*const fn ([*:0]const u8) callconv(.c) ?*const anyopaque,
        egl_gp: ?*const fn ([*:0]const u8) callconv(.c) ?*const anyopaque,
        lib: *std.DynLib,
        name: [*:0]const u8,
    ) ?*const anyopaque {
        if (glx_gp) |gp| {
            if (gp(name)) |p| return p;
        }
        if (egl_gp) |ep| {
            if (ep(name)) |p| return p;
        }
        return lib.lookup(?*const anyopaque, std.mem.span(name)) orelse null;
    }

    fn compileShader(p: *const Gl33Procs, kind: GLenum, src: [*:0]const u8) GLuint {
        const cs = p.glCreateShader orelse return 0;
        const ss = p.glShaderSource orelse return 0;
        const comp = p.glCompileShader orelse return 0;
        const gs = p.glGetShaderiv orelse return 0;
        const sh = cs(kind);
        if (sh == 0) return 0;
        const arr = [_][*:0]const u8{src};
        ss(sh, 1, &arr, null);
        comp(sh);
        var ok: GLint = 0;
        gs(sh, GL_COMPILE_STATUS, &ok);
        if (ok == 0) {
            if (p.glDeleteShader) |ds| ds(sh);
            return 0;
        }
        return sh;
    }

    fn buildDefaultProgram(p: *const Gl33Procs, is_gles: bool) GLuint {
        const cp = p.glCreateProgram orelse return 0;
        const att = p.glAttachShader orelse return 0;
        const lp = p.glLinkProgram orelse return 0;
        const gp = p.glGetProgramiv orelse return 0;
        const vs = compileShader(p, GL_VERTEX_SHADER, if (is_gles) VS_SRC_ES300 else VS_SRC_330);
        const fs = compileShader(p, GL_FRAGMENT_SHADER, if (is_gles) FS_SRC_ES300 else FS_SRC_330);
        if (vs == 0 or fs == 0) {
            if (p.glDeleteShader) |ds| {
                if (vs != 0) ds(vs);
                if (fs != 0) ds(fs);
            }
            return 0;
        }
        const prog = cp();
        if (prog != 0) {
            att(prog, vs);
            att(prog, fs);
            lp(prog);
            var ok: GLint = 0;
            gp(prog, GL_LINK_STATUS, &ok);
            if (ok == 0) {
                if (p.glDeleteProgram) |dp| dp(prog);
                if (p.glDeleteShader) |ds| {
                    ds(vs);
                    ds(fs);
                }
                return 0;
            }
        }
        if (p.glDeleteShader) |ds| {
            ds(vs);
            ds(fs);
        }
        return prog;
    }

    pub fn probe() ?Gl33Backend {
        var is_gles = false;
        var lib = std.DynLib.open("libGL.so.1") catch
            (std.DynLib.open("libOpenGL.so.0") catch blk: {
                is_gles = true;
                break :blk std.DynLib.open("libGLESv3.so") catch return null;
            });

        var lib_egl: ?std.DynLib = std.DynLib.open("libEGL.so.1") catch
            (std.DynLib.open("libEGL.so") catch null);
        const GP = *const fn ([*:0]const u8) callconv(.c) ?*const anyopaque;
        const glx_gp = lib.lookup(GP, "glXGetProcAddressARB") orelse lib.lookup(GP, "glXGetProcAddress");
        const egl_gp = if (lib_egl) |*el| el.lookup(GP, "eglGetProcAddress") else null;

        var p = Gl33Procs{};
        p.glGetString = lib.lookup(@TypeOf(p.glGetString.?), "glGetString");
        p.glGetIntegerv = lib.lookup(@TypeOf(p.glGetIntegerv.?), "glGetIntegerv");
        p.glViewport = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glViewport")));
        p.glClearColor = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glClearColor")));
        p.glClearDepth = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glClearDepth")));
        p.glClearDepthf = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glClearDepthf")));
        p.glClear = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glClear")));
        p.glGenBuffers = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glGenBuffers")));
        p.glDeleteBuffers = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glDeleteBuffers")));
        p.glGenTextures = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glGenTextures")));
        p.glDeleteTextures = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glDeleteTextures")));
        p.glActiveTexture = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glActiveTexture")));
        p.glBindTexture = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glBindTexture")));
        p.glGenVertexArrays = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glGenVertexArrays")));
        p.glDeleteVertexArrays = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glDeleteVertexArrays")));
        p.glBindVertexArray = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glBindVertexArray")));
        p.glCreateShader = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glCreateShader")));
        p.glShaderSource = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glShaderSource")));
        p.glCompileShader = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glCompileShader")));
        p.glGetShaderiv = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glGetShaderiv")));
        p.glDeleteShader = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glDeleteShader")));
        p.glCreateProgram = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glCreateProgram")));
        p.glAttachShader = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glAttachShader")));
        p.glLinkProgram = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glLinkProgram")));
        p.glGetProgramiv = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glGetProgramiv")));
        p.glDeleteProgram = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glDeleteProgram")));
        p.glGetUniformLocation = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glGetUniformLocation")));
        p.glUniform1i = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glUniform1i")));
        p.glUseProgram = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glUseProgram")));
        p.glBindBuffer = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glBindBuffer")));
        p.glBufferData = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glBufferData")));
        p.glMapBufferRange = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glMapBufferRange")));
        p.glUnmapBuffer = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glUnmapBuffer")));
        p.glTexBuffer = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glTexBuffer")));
        p.glDrawElementsInstanced = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glDrawElementsInstanced")));
        p.glDrawElementsInstancedBaseVertex = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glDrawElementsInstancedBaseVertex")));
        p.glDrawElementsInstancedBaseVertexBaseInstance = @ptrCast(@alignCast(resolveProc(glx_gp, egl_gp, &lib, "glDrawElementsInstancedBaseVertexBaseInstance")));

        const get_str = p.glGetString orelse {
            if (lib_egl) |*el| el.close();
            lib.close();
            return null;
        };
        if (get_str(GL_VERSION) == null) {
            if (lib_egl) |*el| el.close();
            lib.close();
            return null;
        }
        var maj: i32 = 0;
        var min: i32 = 0;
        if (p.glGetIntegerv) |gi| {
            gi(GL_MAJOR_VERSION, &maj);
            gi(GL_MINOR_VERSION, &min);
        }
        if (maj < 3 or (!is_gles and maj == 3 and min < 3)) {
            if (lib_egl) |*el| el.close();
            lib.close();
            return null;
        }
        var tbo_tex: GLuint = 0;
        if (p.glGenTextures) |gt| gt(1, @ptrCast(&tbo_tex));
        var vao: GLuint = 0;
        if (p.glGenVertexArrays) |gva| {
            gva(1, @ptrCast(&vao));
            if (vao != 0) if (p.glBindVertexArray) |bva| bva(vao);
        }
        const prog = buildDefaultProgram(&p, is_gles);
        var u_first: GLint = -1;
        if (prog != 0) {
            if (p.glUseProgram) |up| up(prog);
            if (p.glGetUniformLocation) |gul| {
                u_first = gul(prog, "uFirstInstance");
                const tbo_loc = gul(prog, "uInstances");
                if (tbo_loc >= 0) if (p.glUniform1i) |u1i| u1i(tbo_loc, 0);
            }
        }

        return Gl33Backend{
            .lib_gl = lib,
            .lib_egl = lib_egl,
            .procs = p,
            .major = maj,
            .minor = min,
            .has_base_instance = (maj > 4 or (maj == 4 and min >= 2)) and p.glDrawElementsInstancedBaseVertexBaseInstance != null,
            .gl_tbo_tex = tbo_tex,
            .gl_default_vao = vao,
            .gl_default_prog = prog,
            .u_first_inst_loc = u_first,
        };
    }

    pub fn deinit(self: *Gl33Backend) void {
        if (self.procs.glDeleteProgram) |dp| {
            if (self.gl_default_prog != 0) dp(self.gl_default_prog);
        }
        if (self.procs.glDeleteVertexArrays) |dva| {
            if (self.gl_default_vao != 0) dva(1, @ptrCast(&self.gl_default_vao));
        }
        if (self.procs.glDeleteTextures) |dt| {
            if (self.gl_tbo_tex != 0) dt(1, @ptrCast(&self.gl_tbo_tex));
        }
        if (self.procs.glDeleteBuffers) |db| {
            for (self.gl_buffers) |bid| {
                if (bid != 0) db(1, @ptrCast(&bid));
            }
        }
        if (self.lib_egl) |*el| el.close();
        self.lib_gl.close();
    }

    pub fn onCreateBuffer(self: *Gl33Backend, id: u32, desc: rhi.BufferDesc) void {
        if (id >= MAX_HANDLES) return;
        const gb = self.procs.glGenBuffers orelse return;
        var gl_id: GLuint = 0;
        gb(1, @ptrCast(&gl_id));
        if (gl_id == 0) return;
        self.gl_buffers[id] = gl_id;
        if (self.procs.glBindBuffer) |bb| {
            const target: GLenum = if (desc.usage.index) GL_ELEMENT_ARRAY_BUFFER else if (desc.usage.storage) GL_TEXTURE_BUFFER else GL_ARRAY_BUFFER;
            bb(target, gl_id);
            if (self.procs.glBufferData) |bd| {
                bd(target, @intCast(desc.size), null, if (desc.usage.host_visible) GL_STREAM_DRAW else GL_STATIC_DRAW);
            }
        }
    }

    fn ensureRingGlBuffer(self: *Gl33Backend, buf_id: u32, ring_bytes: usize) GLuint {
        if (buf_id >= MAX_HANDLES) return 0;
        if (self.gl_buffers[buf_id] == 0) {
            if (self.procs.glGenBuffers) |gb| {
                var bid: GLuint = 0;
                gb(1, @ptrCast(&bid));
                self.gl_buffers[buf_id] = bid;
                if (bid != 0) {
                    if (self.procs.glBindBuffer) |bb| bb(GL_TEXTURE_BUFFER, bid);
                    if (self.procs.glBufferData) |bd| bd(GL_TEXTURE_BUFFER, @intCast(ring_bytes), null, GL_STREAM_DRAW);
                }
            }
        }
        return self.gl_buffers[buf_id];
    }

    fn dispatchInstanced(self: *const Gl33Backend, index_count: u32, instance_count: u32, first_index: u32, vertex_offset: i32, first_instance: u32) void {
        const idx_off: ?*const anyopaque = @ptrFromInt(@as(usize, first_index) * @sizeOf(u32));
        if (self.has_base_instance) {
            if (self.procs.glDrawElementsInstancedBaseVertexBaseInstance) |fn_bi| {
                fn_bi(GL_TRIANGLES, @intCast(index_count), GL_UNSIGNED_INT, idx_off, @intCast(instance_count), vertex_offset, first_instance);
                return;
            }
        }
        if (self.u_first_inst_loc >= 0) {
            if (self.procs.glUniform1i) |u1i| u1i(self.u_first_inst_loc, @intCast(first_instance));
        }
        if (self.procs.glDrawElementsInstancedBaseVertex) |fn_bv| {
            fn_bv(GL_TRIANGLES, @intCast(index_count), GL_UNSIGNED_INT, idx_off, @intCast(instance_count), vertex_offset);
        } else if (vertex_offset == 0) {
            if (self.procs.glDrawElementsInstanced) |fn_ei| {
                fn_ei(GL_TRIANGLES, @intCast(index_count), GL_UNSIGNED_INT, idx_off, @intCast(instance_count));
            }
        }
    }

    /// Streams TBO (`GL_TEXTURE_BUFFER`) instance payloads via orphan+unsync mapping without pipeline stalls
    /// and lowers indirect multi-draws cleanly into OpenGL 3.3 Core instanced draws.
    pub fn executeCommands(self: *Gl33Backend, cmds: []const rhi.Command, ring: *rhi.FrameRing) void {
        for (cmds) |cmd| {
            switch (cmd) {
                .begin_pass => |bp| {
                    if (self.procs.glViewport) |vp| vp(0, 0, @intCast(bp.viewport_w), @intCast(bp.viewport_h));
                    if (self.procs.glClearColor) |cc| cc(bp.clear_r, bp.clear_g, bp.clear_b, bp.clear_a);
                    if (self.procs.glClearDepthf) |cdf| {
                        cdf(bp.clear_depth);
                    } else if (self.procs.glClearDepth) |cd| {
                        cd(@floatCast(bp.clear_depth));
                    }
                    if (self.procs.glClear) |cl| cl(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
                },
                .bind_pipeline => |_| {
                    if (self.gl_default_vao != 0) {
                        if (self.procs.glBindVertexArray) |bva| bva(self.gl_default_vao);
                    }
                    if (self.gl_default_prog != 0) {
                        if (self.procs.glUseProgram) |up| up(self.gl_default_prog);
                    }
                },
                .bind_index_buffer => |ib| {
                    const id = @intFromEnum(ib.buf);
                    if (id < MAX_HANDLES and self.gl_buffers[id] != 0) {
                        if (self.procs.glBindBuffer) |bb| bb(GL_ELEMENT_ARRAY_BUFFER, self.gl_buffers[id]);
                    }
                },
                .bind_instance_buffer => |ib| {
                    self.tbo_uploads += 1;
                    const bid = @intFromEnum(ib.buf);
                    const slot = if (bid > 0 and bid <= rhi.FrameRing.FRAMES_IN_FLIGHT) bid - 1 else continue;
                    const gl_id = self.ensureRingGlBuffer(bid, ring.capacity_per_frame);
                    if (gl_id == 0) continue;
                    if (self.procs.glBindBuffer) |bb| bb(GL_TEXTURE_BUFFER, gl_id);
                    if (self.procs.glMapBufferRange) |mbr| {
                        const upload_sz = if (ib.size_bytes > 0) ib.size_bytes else (ring.cursor -| ib.offset_bytes);
                        if (upload_sz > 0 and upload_sz <= ring.capacity_per_frame) {
                            // Orphan buffer cleanly then map at GL offset 0 so texelFetch(uInstances, base)
                            // indexes directly from instance 0 regardless of the host staging ring offset.
                            if (self.procs.glBufferData) |bd| bd(GL_TEXTURE_BUFFER, @intCast(upload_sz), null, GL_STREAM_DRAW);
                            if (mbr(GL_TEXTURE_BUFFER, 0, @intCast(upload_sz), GL_MAP_WRITE_BIT | GL_MAP_UNSYNCHRONIZED_BIT)) |dst_ptr| {
                                const dst: [*]u8 = @ptrCast(dst_ptr);
                                const src = ring.ring_mem[slot].ptr + ib.offset_bytes;
                                @memcpy(dst[0..upload_sz], src[0..upload_sz]);
                                if (self.procs.glUnmapBuffer) |ub| _ = ub(GL_TEXTURE_BUFFER);
                            }
                        }
                    }
                    if (self.gl_tbo_tex != 0) {
                        if (self.procs.glActiveTexture) |at| at(GL_TEXTURE0 + ib.binding);
                        if (self.procs.glBindTexture) |bt| bt(GL_TEXTURE_BUFFER, self.gl_tbo_tex);
                        if (self.procs.glTexBuffer) |tb| tb(GL_TEXTURE_BUFFER, GL_RGBA32F, gl_id);
                    }
                },
                .draw_indexed_instanced => |d| {
                    self.lowered_instanced_draws += 1;
                    if (self.gl_default_vao != 0) {
                        if (self.procs.glBindVertexArray) |bva| bva(self.gl_default_vao);
                    }
                    self.dispatchInstanced(d.index_count, d.instance_count, d.first_index, d.vertex_offset, d.first_instance);
                },
                .draw_indexed_indirect_range => |ir| {
                    self.lowered_instanced_draws += ir.fallback_cmds.len;
                    if (self.gl_default_vao != 0) {
                        if (self.procs.glBindVertexArray) |bva| bva(self.gl_default_vao);
                    }
                    for (ir.fallback_cmds) |fc| {
                        self.dispatchInstanced(fc.index_count, fc.instance_count, fc.first_index, fc.vertex_offset, fc.first_instance);
                    }
                },
                else => {},
            }
        }
    }
};
