package org.lwjgl.opengl;

public final class ARBBaseInstance {
    private ARBBaseInstance() {}

    // OpenGL ES 3.2 has no base instance (GLCapabilities reports GL_ARB_base_instance absent). Base instance 0 is the
    // plain instanced draw; any other value cannot be expressed and must not be dropped silently. (The check is
    // inlined: a new method would change this class's facade in every shard that calls it.)
    public static void glDrawArraysInstancedBaseInstance(int mode, int first, int count, int primcount, int baseinstance) {
        if (baseinstance != 0) throw new UnsupportedOperationException("base instance is not available on OpenGL ES");
        GL33C.glDrawArraysInstanced(mode, first, count, primcount);
    }

    public static void glDrawElementsInstancedBaseVertexBaseInstance(
            int mode, int count, int type, long indices, int primcount, int basevertex, int baseinstance) {
        if (baseinstance != 0) throw new UnsupportedOperationException("base instance is not available on OpenGL ES");
        GL33C.glDrawElementsInstancedBaseVertex(mode, count, type, indices, primcount, basevertex);
    }
}
