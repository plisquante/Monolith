package j2z.rhi;

import java.nio.ByteBuffer;

/**
 * Built-in Zero-LWJGL RHI, World Render & GPU Instancing bridge for J2ZIG client targets.
 * Eliminates external LWJGL/FFM native libraries cleanly via J2ZIG's Vulkan 1.1+ / OpenGL 3.3 Core RHI.
 */
public final class NativeRhi {
    public static final int BACKEND_AUTO = 0;
    public static final int BACKEND_VULKAN = 1;
    public static final int BACKEND_OPENGL33 = 2;
    public static final int BACKEND_HEADLESS = 3;

    public static final int POWER_MAX_FPS = 0;
    public static final int POWER_BALANCED = 1;
    public static final int POWER_ECO_BATTERY = 2;

    public static final int KIND_BLOCK_DISPLAY = 0;
    public static final int KIND_ITEM_DISPLAY = 1;
    public static final int KIND_TEXT_DISPLAY = 2;
    public static final int KIND_MOB = 3;
    public static final int KIND_PLAYER = 4;
    public static final int KIND_DROPPED_ITEM = 5;
    public static final int KIND_CHEST = 6;
    public static final int KIND_SIGN = 7;
    public static final int KIND_PARTICLE = 8;

    public static final int BILLBOARD_FIXED = 0;
    public static final int BILLBOARD_VERTICAL = 1;
    public static final int BILLBOARD_HORIZONTAL = 2;
    public static final int BILLBOARD_CENTER = 3;

    public static final long PORTAL_ALL_OPEN = (1L << 36) - 1L;

    private NativeRhi() {}

    public static int init(int backend, int powerProfile) {
        return init0(backend, powerProfile);
    }

    public static long beginFrame(int width, int height) {
        return beginFrame0(width, height);
    }

    public static void endFrame() {
        endFrame0();
    }

    public static void pushArenaScope() {
        pushArenaScope0();
    }

    public static void popArenaScope() {
        popArenaScope0();
    }

    public static void clearSections() {
        clearSections0();
    }

    public static void submitSectionSnapshot(int sx, int sy, int sz, boolean hasGeometry, boolean opaqueOccluder, long portalBits) {
        submitSectionSnapshot0(sx, sy, sz, hasGeometry ? 1 : 0, opaqueOccluder ? 1 : 0, portalBits);
    }

    public static void submitSectionSnapshotEx(int sx, int sy, int sz, boolean hasGeometry, boolean opaqueOccluder, long portalBits,
                                               int indexCount, int firstIndex, int vertexOffset) {
        submitSectionSnapshotEx0(sx, sy, sz, hasGeometry ? 1 : 0, opaqueOccluder ? 1 : 0, portalBits, indexCount, firstIndex, vertexOffset);
    }

    public static int cullWorld(float camX, float camY, float camZ, float lookX, float lookY, float lookZ, float fovDeg, float minPixelArea) {
        return cullWorld0(camX, camY, camZ, lookX, lookY, lookZ, fovDeg, minPixelArea);
    }

    public static int visibleSectionCount() {
        return visibleSectionCount0();
    }

    public static int visibleSectionField(int index, int field) {
        return visibleSectionField0(index, field);
    }

    public static void glViewport(int x, int y, int width, int height) {
        glViewport0(x, y, width, height);
    }

    public static void glClearColor(float r, float g, float b, float a) {
        glClearColor0(r, g, b, a);
    }

    public static void glClear(int mask) {
        glClear0(mask);
    }

    public static void registerMesh(int meshId, int indexCount, int firstIndex, int vertexOffset) {
        registerMesh0(meshId, indexCount, firstIndex, vertexOffset);
    }

    public static void pushInstance(int kind, int meshId, float x, float y, float z, float scale, float radius, int billboard, int uvRect, int colorLight) {
        pushInstance0(kind, meshId, x, y, z, scale, radius, billboard, uvRect, colorLight);
    }

    public static int flushInstances(float camX, float camY, float camZ, float lookX, float lookY, float lookZ, float fovDeg, float minPixelArea) {
        return flushInstances0(camX, camY, camZ, lookX, lookY, lookZ, fovDeg, minPixelArea);
    }

    public static int lastDrawBatches() {
        return lastDrawBatches0();
    }

    /** Committed bytes of the calling thread's transient arena; poll from the render thread. */
    public static long arenaBytes() {
        return arenaBytes0();
    }

    /** Committed bytes of the global segregated SlabPool (all persistent small objects). */
    public static long slabCommittedBytes() {
        return slabCommittedBytes0();
    }

    /** Bytes already returned to the SlabPool by the 3-epoch reclamation machinery. */
    public static long slabReclaimedBytes() {
        return slabReclaimedBytes0();
    }

    /** Monotonically increasing frame epoch (advances in {@link #beginFrame(int,int)}). */
    public static long frameEpoch() {
        return frameEpoch0();
    }

    /** Live OS threads registered in the J2ZIG runtime. */
    public static long threadCount() {
        return threadCount0();
    }

    /** Section snapshots dropped after the bridge reached its 65536 submission cap. */
    public static long droppedSections() {
        return droppedSections0();
    }

    // ---- Android Surface / Input / Direct GL->GLES3 Dispatch ----
    public static boolean hasOsSurface() { return hasOsSurface0() != 0; }
    public static boolean attachOsSurface() { return attachOsSurface0() != 0; }
    public static void swapOsSurface() { swapOsSurface0(); }
    public static int osSurfaceWidth() { return osSurfaceWidth0(); }
    public static int osSurfaceHeight() { return osSurfaceHeight0(); }
    public static void pollOsInput(int[] btns22, float[] axes4) { pollOsInput0(btns22, axes4); }
    /** Typed text and editing keys queued by the host: triples (kind, a, b); kind 1 = character (code point), kind 2 = key (SDL keycode, 1 down / 0 up / 2 repeat). Returns the number of triples. */
    public static int pollText(int[] out) { return pollText0(out); }
    /** Tells the host whether the game holds the mouse (playing) or shows its cursor (a screen is open). */
    public static void setMouseGrabbed(boolean grabbed) { setMouseGrabbed0(grabbed ? 1 : 0); }

    public static void glEnable(int cap) { glEnable0(cap); }
    public static void glDisable(int cap) { glDisable0(cap); }
    public static void glBlendFunc(int sfactor, int dfactor) { glBlendFunc0(sfactor, dfactor); }
    public static void glDepthFunc(int func) { glDepthFunc0(func); }
    public static void glDepthMask(boolean flag) { glDepthMask0(flag ? 1 : 0); }
    public static void glColorMask(boolean r, boolean g, boolean b, boolean a) { glColorMask0(r ? 1 : 0, g ? 1 : 0, b ? 1 : 0, a ? 1 : 0); }
    public static void glCullFace(int mode) { glCullFace0(mode); }
    public static void glFrontFace(int mode) { glFrontFace0(mode); }
    public static void glScissor(int x, int y, int w, int h) { glScissor0(x, y, w, h); }
    public static void glClearDepth(double d) { glClearDepth0(d); }
    public static void glDepthRange(double n, double f) { glDepthRange0(n, f); }
    public static void glPolygonMode(int face, int mode) { glPolygonMode0(face, mode); }
    public static int glGetError() { return glGetError0(); }
    public static int glGetInteger(int pname) { return glGetInteger0(pname); }
    public static String glGetString(int name) { return glGetString0(name); }

    public static int glGenBuffers() { return glGenBuffer0(); }
    public static void glDeleteBuffers(int id) { glDeleteBuffer0(id); }
    public static void glBindBuffer(int target, int id) { glBindBuffer0(target, id); }
    public static void glBufferData(int target, long addr, long size, int usage) { glBufferData0(target, addr, size, usage); }
    public static void glBufferSubData(int target, long offset, long addr, long size) { glBufferSubData0(target, offset, addr, size); }

    public static int glGenVertexArrays() { return glGenVertexArray0(); }
    public static void glDeleteVertexArrays(int id) { glDeleteVertexArray0(id); }
    public static void glBindVertexArray(int id) { glBindVertexArray0(id); }
    public static void glEnableVertexAttribArray(int index) { glEnableVertexAttribArray0(index); }
    public static void glDisableVertexAttribArray(int index) { glDisableVertexAttribArray0(index); }
    public static void glVertexAttribPointer(int index, int size, int type, boolean normalized, int stride, long pointer) {
        glVertexAttribPointer0(index, size, type, normalized ? 1 : 0, stride, pointer);
    }
    public static void glVertexAttribIPointer(int index, int size, int type, int stride, long pointer) {
        glVertexAttribIPointer0(index, size, type, stride, pointer);
    }

    public static int glCreateShader(int type) { return glCreateShader0(type); }
    public static void glDeleteShader(int shader) { glDeleteShader0(shader); }
    public static void glShaderSource(int shader, String src) { glShaderSource0(shader, src); }
    public static void glCompileShader(int shader) { glCompileShader0(shader); }
    public static int glGetShaderi(int shader, int pname) { return glGetShaderi0(shader, pname); }

    public static int glCreateProgram() { return glCreateProgram0(); }
    public static void glDeleteProgram(int program) { glDeleteProgram0(program); }
    public static void glAttachShader(int program, int shader) { glAttachShader0(program, shader); }
    public static void glLinkProgram(int program) { glLinkProgram0(program); }
    public static void glUseProgram(int program) { glUseProgram0(program); }
    public static int glGetProgrami(int program, int pname) { return glGetProgrami0(program, pname); }
    public static int glGetUniformLocation(int program, String name) { return glGetUniformLocation0(program, name); }
    public static void glUniform1i(int loc, int v0) { glUniform1i0(loc, v0); }
    public static void glUniform1f(int loc, float v0) { glUniform1f0(loc, v0); }
    public static void glUniform2f(int loc, float v0, float v1) { glUniform2f0(loc, v0, v1); }
    public static void glUniform3f(int loc, float v0, float v1, float v2) { glUniform3f0(loc, v0, v1, v2); }
    public static void glUniform4f(int loc, float v0, float v1, float v2, float v3) { glUniform4f0(loc, v0, v1, v2, v3); }
    public static void glUniformMatrix4fv(int loc, int count, boolean transpose, long ptr) { glUniformMatrix4fv0(loc, count, transpose ? 1 : 0, ptr); }

    public static int glGenTextures() { return glGenTexture0(); }
    public static void glDeleteTextures(int id) { glDeleteTexture0(id); }
    public static void glActiveTexture(int texture) { glActiveTexture0(texture); }
    public static void glBindTexture(int target, int id) { glBindTexture0(target, id); }
    public static void glTexParameteri(int target, int pname, int param) { glTexParameteri0(target, pname, param); }
    public static void glTexImage2D(int target, int level, int internalformat, int width, int height, int border, int format, int type, long ptr) {
        glTexImage2D0(target, level, internalformat, width, height, border, format, type, ptr);
    }
    public static void glTexSubImage2D(int target, int level, int xoff, int yoff, int width, int height, int format, int type, long ptr) {
        glTexSubImage2D0(target, level, xoff, yoff, width, height, format, type, ptr);
    }

    public static int glGenFramebuffers() { return glGenFramebuffer0(); }
    public static void glDeleteFramebuffers(int id) { glDeleteFramebuffer0(id); }
    public static void glBindFramebuffer(int target, int id) { glBindFramebuffer0(target, id); }
    public static void glFramebufferTexture2D(int target, int att, int texTarget, int tex, int level) { glFramebufferTexture2D0(target, att, texTarget, tex, level); }
    public static int glCheckFramebufferStatus(int target) { return glCheckFramebufferStatus0(target); }

    public static void glDrawArrays(int mode, int first, int count) { glDrawArrays0(mode, first, count); }
    public static void glDrawElements(int mode, int count, int type, long offset) { glDrawElements0(mode, count, type, offset); }
    public static void glDrawElementsInstanced(int mode, int count, int type, long offset, int primcount) { glDrawElementsInstanced0(mode, count, type, offset, primcount); }
    public static void glDrawArraysInstanced(int mode, int first, int count, int primcount) { glDrawArraysInstanced0(mode, first, count, primcount); }
    public static void glDrawElementsInstancedBaseVertex(int mode, int count, int type, long offset, int primcount, int baseVertex) {
        glDrawElementsInstancedBaseVertex0(mode, count, type, offset, primcount, baseVertex);
    }
    public static void glBlitFramebuffer(int sx0, int sy0, int sx1, int sy1, int dx0, int dy0, int dx1, int dy1, int mask, int filter) {
        glBlitFramebuffer0(sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, mask, filter);
    }
    public static void glBindAttribLocation(int prog, int idx, String name) { glBindAttribLocation0(prog, idx, name); }
    public static void glBindBufferRange(int target, int idx, int buf, long offset, long size) { glBindBufferRange0(target, idx, buf, offset, size); }
    public static void glCopyBufferSubData(int rt, int wt, long ro, long wo, long sz) { glCopyBufferSubData0(rt, wt, ro, wo, sz); }
    public static long glMapBufferRange(int target, long offset, long length, int access) { return glMapBufferRange0(target, offset, length, access); }
    public static boolean glUnmapBuffer(int target) { return glUnmapBuffer0(target) != 0; }
    public static void glFlushMappedBufferRange(int target, long offset, long length) { glFlushMappedBufferRange0(target, offset, length); }
    public static int glGenSamplers() { return glGenSampler0(); }
    public static void glDeleteSamplers(int id) { glDeleteSampler0(id); }
    public static void glBindSampler(int unit, int sampler) { glBindSampler0(unit, sampler); }
    public static void glSamplerParameteri(int sampler, int pname, int param) { glSamplerParameteri0(sampler, pname, param); }
    public static void glSamplerParameterf(int sampler, int pname, float param) { glSamplerParameterf0(sampler, pname, param); }
    public static void glBlendEquationSeparate(int modeRGB, int modeAlpha) { glBlendEquationSeparate0(modeRGB, modeAlpha); }
    public static void glBlendFuncSeparate(int sRGB, int dRGB, int sAlpha, int dAlpha) { glBlendFuncSeparate0(sRGB, dRGB, sAlpha, dAlpha); }
    public static void glClearBufferfv(int buf, int drawbuffer, float[] value) { glClearBufferfv0(buf, drawbuffer, value); }
    public static void glColorMaski(int buf, boolean r, boolean g, boolean b, boolean a) { glColorMaski0(buf, r ? 1 : 0, g ? 1 : 0, b ? 1 : 0, a ? 1 : 0); }
    public static void glEnablei(int target, int index) { glEnablei0(target, index); }
    public static void glDisablei(int target, int index) { glDisablei0(target, index); }
    public static void glDrawBuffers(int[] bufs) { glDrawBuffers0(bufs); }
    public static void glReadBuffer(int src) { glReadBuffer0(src); }
    public static void glFinish() { glFinish0(); }
    public static float glGetFloat(int pname) { return glGetFloat0(pname); }
    public static long glGetInteger64(int pname) { return glGetInteger640(pname); }
    public static String glGetShaderInfoLog(int sh) { return glGetShaderInfoLog0(sh); }
    public static String glGetProgramInfoLog(int prog) { return glGetProgramInfoLog0(prog); }
    public static int glGetUniformBlockIndex(int prog, String name) { return glGetUniformBlockIndex0(prog, name); }
    public static void glUniformBlockBinding(int prog, int idx, int binding) { glUniformBlockBinding0(prog, idx, binding); }
    public static void glPixelStorei(int pname, int param) { glPixelStorei0(pname, param); }
    public static void glPolygonOffset(float factor, float units) { glPolygonOffset0(factor, units); }
    public static void glReadPixels(int x, int y, int w, int h, int fmt, int typ, long ptr) { glReadPixels0(x, y, w, h, fmt, typ, ptr); }
    public static void glTexBuffer(int target, int ifmt, int buf) { glTexBuffer0(target, ifmt, buf); }
    public static int glGetTexLevelParameteri(int target, int level, int pname) { return glGetTexLevelParameteri0(target, level, pname); }
    public static int glGenQueries() { return glGenQuery0(); }
    public static void glDeleteQueries(int id) { glDeleteQuery0(id); }
    public static void glQueryCounter(int id, int target) { glQueryCounter0(id, target); }
    public static int glGetQueryObjecti(int id, int pname) { return glGetQueryObjecti0(id, pname); }
    public static long glGetQueryObjectui64(int id, int pname) { return glGetQueryObjectui640(id, pname); }
    public static void glVertexAttribDivisor(int index, int divisor) { glVertexAttribDivisor0(index, divisor); }
    public static long glFenceSync(int condition, int flags) { return glFenceSync0(condition, flags); }
    public static void glDeleteSync(long sync) { glDeleteSync0(sync); }
    public static int glClientWaitSync(long sync, int flags, long timeoutNs) { return glClientWaitSync0(sync, flags, timeoutNs); }

    public static long alcOpenDevice(String dev) { return alcOpenDevice0(dev); }
    public static boolean alcCloseDevice(long dev) { return alcCloseDevice0(dev) != 0; }
    public static long alcCreateContext(long dev, long attrPtr) { return alcCreateContext0(dev, attrPtr); }
    public static boolean alcMakeContextCurrent(long ctx) { return alcMakeContextCurrent0(ctx) != 0; }
    public static void alcDestroyContext(long ctx) { alcDestroyContext0(ctx); }
    public static int alcGetInteger(long dev, int param) { return alcGetInteger0(dev, param); }
    public static String alcGetString(long dev, int param) { return alcGetString0(dev, param); }
    public static int alGenBuffer() { return alGenBuffer0(); }
    public static void alDeleteBuffer(int id) { alDeleteBuffer0(id); }
    public static void alBufferData(int buf, int fmt, long addr, int size, int freq) { alBufferData0(buf, fmt, addr, size, freq); }
    public static int alGenSource() { return alGenSource0(); }
    public static void alDeleteSource(int id) { alDeleteSource0(id); }
    public static void alSourcePlay(int src) { alSourcePlay0(src); }
    public static void alSourcePause(int src) { alSourcePause0(src); }
    public static void alSourceStop(int src) { alSourceStop0(src); }
    public static void alSourceQueueBuffer(int src, int buf) { alSourceQueueBuffer0(src, buf); }
    public static int alSourceUnqueueBuffer(int src) { return alSourceUnqueueBuffer0(src); }
    public static int alGetSourcei(int src, int param) { return alGetSourcei0(src, param); }
    public static void alSourcei(int src, int param, int val) { alSourcei0(src, param, val); }
    public static void alSourcef(int src, int param, float val) { alSourcef0(src, param, val); }
    public static void alSource3f(int src, int param, float x, float y, float z) { alSource3f0(src, param, x, y, z); }
    public static void alListener3f(int param, float x, float y, float z) { alListener3f0(param, x, y, z); }
    public static int alGetError() { return alGetError0(); }
    public static ByteBuffer wrapDirectByteBuffer(long addr, int cap) { return wrapDirectByteBuffer0(addr, cap); }

    private static native int hasOsSurface0();
    private static native int attachOsSurface0();
    private static native void swapOsSurface0();
    private static native int osSurfaceWidth0();
    private static native int osSurfaceHeight0();
    private static native void pollOsInput0(int[] btns22, float[] axes4);
    private static native int pollText0(int[] out);
    private static native void setMouseGrabbed0(int grabbed);

    private static native void glEnable0(int cap);
    private static native void glDisable0(int cap);
    private static native void glBlendFunc0(int sfactor, int dfactor);
    private static native void glDepthFunc0(int func);
    private static native void glDepthMask0(int flag);
    private static native void glColorMask0(int r, int g, int b, int a);
    private static native void glCullFace0(int mode);
    private static native void glFrontFace0(int mode);
    private static native void glScissor0(int x, int y, int w, int h);
    private static native void glClearDepth0(double d);
    private static native void glDepthRange0(double n, double f);
    private static native void glPolygonMode0(int face, int mode);
    private static native int glGetError0();
    private static native int glGetInteger0(int pname);
    private static native String glGetString0(int name);
    private static native int glGenBuffer0();
    private static native void glDeleteBuffer0(int id);
    private static native void glBindBuffer0(int target, int id);
    private static native void glBufferData0(int target, long addr, long size, int usage);
    private static native void glBufferSubData0(int target, long offset, long addr, long size);
    private static native int glGenVertexArray0();
    private static native void glDeleteVertexArray0(int id);
    private static native void glBindVertexArray0(int id);
    private static native void glEnableVertexAttribArray0(int index);
    private static native void glDisableVertexAttribArray0(int index);
    private static native void glVertexAttribPointer0(int index, int size, int type, int norm, int stride, long pointer);
    private static native void glVertexAttribIPointer0(int index, int size, int type, int stride, long pointer);
    private static native int glCreateShader0(int type);
    private static native void glDeleteShader0(int shader);
    private static native void glShaderSource0(int shader, String src);
    private static native void glCompileShader0(int shader);
    private static native int glGetShaderi0(int shader, int pname);
    private static native int glCreateProgram0();
    private static native void glDeleteProgram0(int program);
    private static native void glAttachShader0(int program, int shader);
    private static native void glLinkProgram0(int program);
    private static native void glUseProgram0(int program);
    private static native int glGetProgrami0(int program, int pname);
    private static native int glGetUniformLocation0(int program, String name);
    private static native void glUniform1i0(int loc, int v0);
    private static native void glUniform1f0(int loc, float v0);
    private static native void glUniform2f0(int loc, float v0, float v1);
    private static native void glUniform3f0(int loc, float v0, float v1, float v2);
    private static native void glUniform4f0(int loc, float v0, float v1, float v2, float v3);
    private static native void glUniformMatrix4fv0(int loc, int count, int transpose, long ptr);
    private static native int glGenTexture0();
    private static native void glDeleteTexture0(int id);
    private static native void glActiveTexture0(int texture);
    private static native void glBindTexture0(int target, int id);
    private static native void glTexParameteri0(int target, int pname, int param);
    private static native void glTexImage2D0(int target, int level, int internalformat, int width, int height, int border, int format, int type, long ptr);
    private static native void glTexSubImage2D0(int target, int level, int xoff, int yoff, int width, int height, int format, int type, long ptr);
    private static native int glGenFramebuffer0();
    private static native void glDeleteFramebuffer0(int id);
    private static native void glBindFramebuffer0(int target, int id);
    private static native void glFramebufferTexture2D0(int target, int att, int texTarget, int tex, int level);
    private static native int glCheckFramebufferStatus0(int target);
    private static native void glDrawArrays0(int mode, int first, int count);
    private static native void glDrawElements0(int mode, int count, int type, long offset);
    private static native void glDrawElementsInstanced0(int mode, int count, int type, long offset, int primcount);
    private static native void glDrawArraysInstanced0(int mode, int first, int count, int primcount);
    private static native void glDrawElementsInstancedBaseVertex0(int mode, int count, int type, long offset, int primcount, int baseVertex);
    private static native void glBlitFramebuffer0(int sx0, int sy0, int sx1, int sy1, int dx0, int dy0, int dx1, int dy1, int mask, int filter);
    private static native void glBindAttribLocation0(int prog, int idx, String name);
    private static native void glBindBufferRange0(int target, int idx, int buf, long offset, long size);
    private static native void glCopyBufferSubData0(int rt, int wt, long ro, long wo, long sz);
    private static native long glMapBufferRange0(int target, long offset, long length, int access);
    private static native int glUnmapBuffer0(int target);
    private static native void glFlushMappedBufferRange0(int target, long offset, long length);
    private static native int glGenSampler0();
    private static native void glDeleteSampler0(int id);
    private static native void glBindSampler0(int unit, int sampler);
    private static native void glSamplerParameteri0(int sampler, int pname, int param);
    private static native void glSamplerParameterf0(int sampler, int pname, float param);
    private static native void glBlendEquationSeparate0(int modeRGB, int modeAlpha);
    private static native void glBlendFuncSeparate0(int sRGB, int dRGB, int sAlpha, int dAlpha);
    private static native void glClearBufferfv0(int buf, int drawbuffer, float[] value);
    private static native void glColorMaski0(int buf, int r, int g, int b, int a);
    private static native void glEnablei0(int target, int index);
    private static native void glDisablei0(int target, int index);
    private static native void glDrawBuffers0(int[] bufs);
    private static native void glReadBuffer0(int src);
    private static native void glFinish0();
    private static native float glGetFloat0(int pname);
    private static native long glGetInteger640(int pname);
    private static native String glGetShaderInfoLog0(int sh);
    private static native String glGetProgramInfoLog0(int prog);
    private static native int glGetUniformBlockIndex0(int prog, String name);
    private static native void glUniformBlockBinding0(int prog, int idx, int binding);
    private static native void glPixelStorei0(int pname, int param);
    private static native void glPolygonOffset0(float factor, float units);
    private static native void glReadPixels0(int x, int y, int w, int h, int fmt, int typ, long ptr);
    private static native void glTexBuffer0(int target, int ifmt, int buf);
    private static native int glGetTexLevelParameteri0(int target, int level, int pname);
    private static native int glGenQuery0();
    private static native void glDeleteQuery0(int id);
    private static native void glQueryCounter0(int id, int target);
    private static native int glGetQueryObjecti0(int id, int pname);
    private static native long glGetQueryObjectui640(int id, int pname);
    private static native void glVertexAttribDivisor0(int index, int divisor);
    private static native long glFenceSync0(int condition, int flags);
    private static native void glDeleteSync0(long sync);
    private static native int glClientWaitSync0(long sync, int flags, long timeoutNs);

    private static native long alcOpenDevice0(String dev);
    private static native int alcCloseDevice0(long dev);
    private static native long alcCreateContext0(long dev, long attrPtr);
    private static native int alcMakeContextCurrent0(long ctx);
    private static native void alcDestroyContext0(long ctx);
    private static native int alcGetInteger0(long dev, int param);
    private static native String alcGetString0(long dev, int param);
    private static native int alGenBuffer0();
    private static native void alDeleteBuffer0(int id);
    private static native void alBufferData0(int buf, int fmt, long addr, int size, int freq);
    private static native int alGenSource0();
    private static native void alDeleteSource0(int id);
    private static native void alSourcePlay0(int src);
    private static native void alSourcePause0(int src);
    private static native void alSourceStop0(int src);
    private static native void alSourceQueueBuffer0(int src, int buf);
    private static native int alSourceUnqueueBuffer0(int src);
    private static native int alGetSourcei0(int src, int param);
    private static native void alSourcei0(int src, int param, int val);
    private static native void alSourcef0(int src, int param, float val);
    private static native void alSource3f0(int src, int param, float x, float y, float z);
    private static native void alListener3f0(int param, float x, float y, float z);
    private static native int alGetError0();
    private static native ByteBuffer wrapDirectByteBuffer0(long addr, int cap);

    private static native int init0(int backend, int powerProfile);
    private static native long beginFrame0(int width, int height);
    private static native void endFrame0();
    private static native void pushArenaScope0();
    private static native void popArenaScope0();
    private static native void clearSections0();
    private static native void submitSectionSnapshot0(int sx, int sy, int sz, int hasGeometry, int opaqueOccluder, long portalBits);
    private static native void submitSectionSnapshotEx0(int sx, int sy, int sz, int hasGeometry, int opaqueOccluder, long portalBits,
                                                        int indexCount, int firstIndex, int vertexOffset);
    private static native int cullWorld0(float camX, float camY, float camZ, float lookX, float lookY, float lookZ, float fovDeg, float minPixelArea);
    private static native int visibleSectionCount0();
    private static native int visibleSectionField0(int index, int field);
    private static native void glViewport0(int x, int y, int width, int height);
    private static native void glClearColor0(float r, float g, float b, float a);
    private static native void glClear0(int mask);
    private static native void registerMesh0(int meshId, int indexCount, int firstIndex, int vertexOffset);
    private static native void pushInstance0(int kind, int meshId, float x, float y, float z, float scale, float radius, int billboard, int uvRect, int colorLight);
    private static native int flushInstances0(float camX, float camY, float camZ, float lookX, float lookY, float lookZ, float fovDeg, float minPixelArea);
    private static native int lastDrawBatches0();
    private static native long arenaBytes0();
    private static native long slabCommittedBytes0();
    private static native long slabReclaimedBytes0();
    private static native long frameEpoch0();
    private static native long threadCount0();
    private static native long droppedSections0();
}
