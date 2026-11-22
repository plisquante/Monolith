package org.lwjgl.opengl;

import mono.rhi.NativeRhi;

public class GL32C extends GL31C {
    public static final int GL_DEPTH_CLAMP = 0x864F;
    public static final int GL_CONTEXT_CORE_PROFILE_BIT = 0x00000001;
    public static final int GL_CONTEXT_PROFILE_MASK = 0x9126;

    protected GL32C() {}

    public static long glFenceSync(int condition, int flags) { return NativeRhi.glFenceSync(condition, flags); }
    public static void glDeleteSync(long sync) { NativeRhi.glDeleteSync(sync); }
    public static int glClientWaitSync(long sync, int flags, long timeout) { return NativeRhi.glClientWaitSync(sync, flags, timeout); }
    public static long glGetInteger64(int pname) { return NativeRhi.glGetInteger64(pname); }
    public static void glDrawElementsInstancedBaseVertex(int mode, int count, int type, long indices, int primcount, int basevertex) {
        NativeRhi.glDrawElementsInstancedBaseVertex(mode, count, type, indices, primcount, basevertex);
    }
}
