package org.lwjgl.opengl;

import mono.rhi.NativeRhi;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;
import java.nio.FloatBuffer;

/** Zero-LWJGL OpenGL 1.5 Buffer Object shim -> NativeRhi / Android GLESv3. */
public class GL15C extends GL11C {
    public static final int GL_ARRAY_BUFFER = 0x8892;
    public static final int GL_ELEMENT_ARRAY_BUFFER = 0x8893;
    public static final int GL_STATIC_DRAW = 0x88E4;
    public static final int GL_DYNAMIC_DRAW = 0x88E8;
    public static final int GL_STREAM_DRAW = 0x88E0;

    protected GL15C() {}

    public static int glGenBuffers() { return NativeRhi.glGenBuffers(); }
    public static void glDeleteBuffers(int buffer) { NativeRhi.glDeleteBuffers(buffer); }
    public static void glBindBuffer(int target, int buffer) { NativeRhi.glBindBuffer(target, buffer); }

    public static void glBufferData(int target, long size, int usage) {
        NativeRhi.glBufferData(target, 0L, size, usage);
    }

    public static void glBufferData(int target, ByteBuffer data, int usage) {
        NativeRhi.glBufferData(target, MemoryUtil.memAddressSafe(data),
                data == null ? 0L : data.remaining(), usage);
    }

    public static void glBufferData(int target, FloatBuffer data, int usage) {
        NativeRhi.glBufferData(target, MemoryUtil.memAddressSafe(data),
                data == null ? 0L : ((long) data.remaining() << 2), usage);
    }

    public static void glBufferSubData(int target, long offset, ByteBuffer data) {
        NativeRhi.glBufferSubData(target, offset, MemoryUtil.memAddressSafe(data),
                data == null ? 0L : data.remaining());
    }
}
