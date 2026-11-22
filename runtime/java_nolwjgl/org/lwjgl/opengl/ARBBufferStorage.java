package org.lwjgl.opengl;

import java.nio.ByteBuffer;

public final class ARBBufferStorage {
    private ARBBufferStorage() {}

    // GLCapabilities reports GL_ARB_buffer_storage as absent (OpenGL ES 3.2 has no immutable buffer storage).
    // Emulating it with glBufferData would break persistent mappings silently, so a call is a hard error.
    public static void glBufferStorage(int target, long size, int flags) {
        throw new UnsupportedOperationException("glBufferStorage: GL_ARB_buffer_storage is not available on OpenGL ES");
    }

    public static void glBufferStorage(int target, ByteBuffer data, int flags) {
        throw new UnsupportedOperationException("glBufferStorage: GL_ARB_buffer_storage is not available on OpenGL ES");
    }
}
