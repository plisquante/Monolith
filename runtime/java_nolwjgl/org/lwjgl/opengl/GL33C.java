package org.lwjgl.opengl;

import j2z.rhi.NativeRhi;
import org.lwjgl.system.MemoryUtil;

public class GL33C extends GL32C {
    protected GL33C() {}

    public static void glVertexAttribDivisor(int index, int divisor) { NativeRhi.glVertexAttribDivisor(index, divisor); }

    public static int glGenSamplers() { return NativeRhi.glGenSamplers(); }
    public static void glDeleteSamplers(int sampler) { NativeRhi.glDeleteSamplers(sampler); }
    public static void glBindSampler(int unit, int sampler) { NativeRhi.glBindSampler(unit, sampler); }
    public static void glSamplerParameteri(int sampler, int pname, int param) { NativeRhi.glSamplerParameteri(sampler, pname, param); }
    public static void glSamplerParameterf(int sampler, int pname, float param) { NativeRhi.glSamplerParameterf(sampler, pname, param); }

    public static void glGenQueries(int[] ids) {
        if (ids == null) return;
        for (int i = 0; i < ids.length; i++) ids[i] = NativeRhi.glGenQueries();
    }
    public static void glDeleteQueries(int[] ids) {
        if (ids == null) return;
        for (int id : ids) NativeRhi.glDeleteQueries(id);
    }
    public static void glQueryCounter(int id, int target) { NativeRhi.glQueryCounter(id, target); }
    public static int glGetQueryObjecti(int id, int pname) { return NativeRhi.glGetQueryObjecti(id, pname); }
    public static long glGetQueryObjectui64(int id, int pname) { return NativeRhi.glGetQueryObjectui64(id, pname); }

    public static void nglMultiDrawArrays(int mode, long first, long count, int drawcount) {
        for (int i = 0; i < drawcount; i++) {
            int f = MemoryUtil.memGetInt(first + (long) i * 4L);
            int c = MemoryUtil.memGetInt(count + (long) i * 4L);
            glDrawArrays(mode, f, c);
        }
    }

    public static void nglMultiDrawElementsBaseVertex(int mode, long count, int type, long indices, int drawcount, long basevertex) {
        for (int i = 0; i < drawcount; i++) {
            int c = MemoryUtil.memGetInt(count + (long) i * 4L);
            long idx = MemoryUtil.memGetAddress(indices + (long) i * 8L);
            int bv = basevertex == 0L ? 0 : MemoryUtil.memGetInt(basevertex + (long) i * 4L);
            glDrawElementsInstancedBaseVertex(mode, c, type, idx, 1, bv);
        }
    }
}
