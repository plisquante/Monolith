package org.lwjgl.opengl;

public final class KHRDebug {
    private KHRDebug() {}

    public static void glDebugMessageCallback(GLDebugMessageCallbackI callback, long userParam) {}
    public static void glDebugMessageControl(int source, int type, int severity, int[] ids, boolean enabled) {}
    public static void glObjectLabel(int identifier, int name, CharSequence label) {}
    public static void glPushDebugGroup(int source, int id, CharSequence message) {}
    public static void glPopDebugGroup() {}
}
