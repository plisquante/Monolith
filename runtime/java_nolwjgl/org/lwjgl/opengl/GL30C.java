package org.lwjgl.opengl;

import j2z.rhi.NativeRhi;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

/** Zero-LWJGL OpenGL 3.0 VAO/FBO/Instancing shim -> NativeRhi / Android GLESv3. */
public class GL30C extends GL20C {
    public static final int GL_FRAMEBUFFER = 0x8D40;
    public static final int GL_COLOR_ATTACHMENT0 = 0x8CE0;
    public static final int GL_DEPTH_ATTACHMENT = 0x8D00;
    public static final int GL_FRAMEBUFFER_COMPLETE = 0x8CD5;

    protected GL30C() {}

    public static int glGenVertexArrays() { return NativeRhi.glGenVertexArrays(); }
    public static void glDeleteVertexArrays(int array) { NativeRhi.glDeleteVertexArrays(array); }
    public static void glBindVertexArray(int array) { NativeRhi.glBindVertexArray(array); }
    public static void glVertexAttribIPointer(int index, int size, int type, int stride, long pointer) {
        NativeRhi.glVertexAttribIPointer(index, size, type, stride, pointer);
    }

    public static int glGenFramebuffers() { return NativeRhi.glGenFramebuffers(); }
    public static void glDeleteFramebuffers(int fb) { NativeRhi.glDeleteFramebuffers(fb); }
    public static void glBindFramebuffer(int target, int fb) { NativeRhi.glBindFramebuffer(target, fb); }
    public static void glFramebufferTexture2D(int target, int attachment, int textarget, int texture, int level) {
        NativeRhi.glFramebufferTexture2D(target, attachment, textarget, texture, level);
    }
    public static int glCheckFramebufferStatus(int target) { return NativeRhi.glCheckFramebufferStatus(target); }

    public static void glDrawElementsInstanced(int mode, int count, int type, long indices, int primcount) {
        NativeRhi.glDrawElementsInstanced(mode, count, type, indices, primcount);
    }
    public static void glBlitFramebuffer(int srcX0, int srcY0, int srcX1, int srcY1,
                                         int dstX0, int dstY0, int dstX1, int dstY1, int mask, int filter) {
        NativeRhi.glBlitFramebuffer(srcX0, srcY0, srcX1, srcY1, dstX0, dstY0, dstX1, dstY1, mask, filter);
    }
    public static void glBindBufferRange(int target, int index, int buffer, long offset, long size) {
        NativeRhi.glBindBufferRange(target, index, buffer, offset, size);
    }
    public static void glClearBufferfv(int buffer, int drawbuffer, float[] value) {
        NativeRhi.glClearBufferfv(buffer, drawbuffer, value);
    }
    public static void glColorMaski(int index, boolean r, boolean g, boolean b, boolean a) {
        NativeRhi.glColorMaski(index, r, g, b, a);
    }
    public static void glEnablei(int cap, int index) { NativeRhi.glEnablei(cap, index); }
    public static void glDisablei(int cap, int index) { NativeRhi.glDisablei(cap, index); }
    public static long nglMapBufferRange(int target, long offset, long length, int access) {
        return NativeRhi.glMapBufferRange(target, offset, length, access);
    }
    public static ByteBuffer glMapBufferRange(int target, long offset, long length, int access) {
        int flags = access;
        if ((flags & 0x1) != 0 && (flags & 0x20) != 0) { // GL_MAP_READ_BIT and GL_MAP_UNSYNCHRONIZED_BIT
            // OpenGL (ES) 3.x: GL_MAP_UNSYNCHRONIZED_BIT together with GL_MAP_READ_BIT is GL_INVALID_OPERATION and the
            // map returns NULL; strict drivers (ANGLE) enforce it. Unsynchronized is only a synchronization hint for
            // writes, so dropping it keeps the result and makes the mapping valid.
            flags &= ~0x20;
        }
        long ptr = NativeRhi.glMapBufferRange(target, offset, length, flags);
        if (ptr == 0L) {
            int err = NativeRhi.glGetError();
            int pname; // binding query of the target, for the report (inlined: no new method in this class's facades)
            switch (target) {
                case 0x8892: pname = 0x8894; break; // ARRAY_BUFFER
                case 0x8893: pname = 0x8895; break; // ELEMENT_ARRAY_BUFFER
                case 0x8F36: pname = 0x8F36; break; // COPY_READ_BUFFER
                case 0x8F37: pname = 0x8F37; break; // COPY_WRITE_BUFFER
                case 0x8A11: pname = 0x8A28; break; // UNIFORM_BUFFER
                case 0x88EB: pname = 0x88ED; break; // PIXEL_PACK_BUFFER
                case 0x88EC: pname = 0x88EF; break; // PIXEL_UNPACK_BUFFER
                default: pname = 0;
            }
            int bound = pname == 0 ? -1 : NativeRhi.glGetInteger(pname);
            // explicit StringBuilder: a string concatenation would add a synthetic class with a new reflective
            // signature, which changes code shared by every shard (see j2zig/shard.py)
            StringBuilder sb = new StringBuilder("j2zig GL: glMapBufferRange failed: error 0x");
            sb.append(Integer.toHexString(err)).append(" target 0x").append(Integer.toHexString(target));
            sb.append(" bound buffer ").append(bound).append(" offset ").append(offset).append(" length ").append(length);
            sb.append(" access 0x").append(Integer.toHexString(access)).append(" (sent 0x").append(Integer.toHexString(flags)).append(')');
            System.err.println(sb.toString());
            return null;
        }
        return MemoryUtil.memByteBuffer(ptr, (int) length);
    }
    public static boolean glUnmapBuffer(int target) { return NativeRhi.glUnmapBuffer(target); }
    public static void glFlushMappedBufferRange(int target, long offset, long length) {
        NativeRhi.glFlushMappedBufferRange(target, offset, length);
    }
}
