package org.lwjgl.opengl;

import java.nio.ByteBuffer;

public final class ARBDirectStateAccess {
    private ARBDirectStateAccess() {}

    public static int glCreateBuffers() { return GL33C.glGenBuffers(); }
    public static int glCreateFramebuffers() { return GL33C.glGenFramebuffers(); }

    public static void glNamedBufferData(int buffer, long size, int usage) {
        GL33C.glBindBuffer(GL15C.GL_ARRAY_BUFFER, buffer);
        GL33C.glBufferData(GL15C.GL_ARRAY_BUFFER, size, usage);
    }

    public static void glNamedBufferData(int buffer, ByteBuffer data, int usage) {
        GL33C.glBindBuffer(GL15C.GL_ARRAY_BUFFER, buffer);
        GL33C.glBufferData(GL15C.GL_ARRAY_BUFFER, data, usage);
    }

    public static void glNamedBufferStorage(int buffer, long size, int flags) {
        glNamedBufferData(buffer, size, GL15C.GL_DYNAMIC_DRAW);
    }

    public static void glNamedBufferStorage(int buffer, ByteBuffer data, int flags) {
        glNamedBufferData(buffer, data, GL15C.GL_DYNAMIC_DRAW);
    }

    public static void glNamedBufferSubData(int buffer, long offset, ByteBuffer data) {
        GL33C.glBindBuffer(GL15C.GL_ARRAY_BUFFER, buffer);
        GL33C.glBufferSubData(GL15C.GL_ARRAY_BUFFER, offset, data);
    }

    public static void glCopyNamedBufferSubData(int readBuffer, int writeBuffer, long readOffset, long writeOffset, long size) {
        GL33C.glBindBuffer(0x8F36, readBuffer);   // GL_COPY_READ_BUFFER
        GL33C.glBindBuffer(0x8F37, writeBuffer);  // GL_COPY_WRITE_BUFFER
        GL33C.glCopyBufferSubData(0x8F36, 0x8F37, readOffset, writeOffset, size);
    }

    public static ByteBuffer glMapNamedBufferRange(int buffer, long offset, long length, int access) {
        GL33C.glBindBuffer(GL15C.GL_ARRAY_BUFFER, buffer);
        return GL33C.glMapBufferRange(GL15C.GL_ARRAY_BUFFER, offset, length, access);
    }

    public static void glFlushMappedNamedBufferRange(int buffer, long offset, long length) {
        GL33C.glBindBuffer(GL15C.GL_ARRAY_BUFFER, buffer);
        GL33C.glFlushMappedBufferRange(GL15C.GL_ARRAY_BUFFER, offset, length);
    }

    public static boolean glUnmapNamedBuffer(int buffer) {
        GL33C.glBindBuffer(GL15C.GL_ARRAY_BUFFER, buffer);
        return GL33C.glUnmapBuffer(GL15C.GL_ARRAY_BUFFER);
    }

    public static void glNamedFramebufferTexture(int framebuffer, int attachment, int texture, int level) {
        GL33C.glBindFramebuffer(GL30C.GL_FRAMEBUFFER, framebuffer);
        GL33C.glFramebufferTexture2D(GL30C.GL_FRAMEBUFFER, attachment, GL11C.GL_TEXTURE_2D, texture, level);
    }

    public static void glBlitNamedFramebuffer(int readFb, int drawFb, int sx0, int sy0, int sx1, int sy1,
                                              int dx0, int dy0, int dx1, int dy1, int mask, int filter) {
        GL33C.glBindFramebuffer(0x8CA8, readFb);  // GL_READ_FRAMEBUFFER
        GL33C.glBindFramebuffer(0x8CA9, drawFb);  // GL_DRAW_FRAMEBUFFER
        GL33C.glBlitFramebuffer(sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, mask, filter);
    }
}
