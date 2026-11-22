package org.lwjgl.opengl;

import mono.rhi.NativeRhi;

public class GL31C extends GL30C {
    public static final int GL_UNIFORM_BUFFER = 0x8A11;
    public static final int GL_TEXTURE_BUFFER = 0x8C2A;

    protected GL31C() {}

    public static void glDrawArraysInstanced(int mode, int first, int count, int primcount) {
        NativeRhi.glDrawArraysInstanced(mode, first, count, primcount);
    }

    public static void glTexBuffer(int target, int internalformat, int buffer) {
        NativeRhi.glTexBuffer(target, internalformat, buffer);
    }

    public static void glCopyBufferSubData(int readTarget, int writeTarget, long readOffset, long writeOffset, long size) {
        NativeRhi.glCopyBufferSubData(readTarget, writeTarget, readOffset, writeOffset, size);
    }

    public static int glGetUniformBlockIndex(int program, CharSequence uniformBlockName) {
        return NativeRhi.glGetUniformBlockIndex(program, uniformBlockName == null ? "" : uniformBlockName.toString());
    }

    public static void glUniformBlockBinding(int program, int uniformBlockIndex, int uniformBlockBinding) {
        NativeRhi.glUniformBlockBinding(program, uniformBlockIndex, uniformBlockBinding);
    }
}
