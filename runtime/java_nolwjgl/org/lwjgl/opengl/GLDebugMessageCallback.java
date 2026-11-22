package org.lwjgl.opengl;

import org.lwjgl.system.Callback;
import org.lwjgl.system.MemoryUtil;

public final class GLDebugMessageCallback extends Callback implements GLDebugMessageCallbackI {
    private final GLDebugMessageCallbackI delegate;

    private GLDebugMessageCallback(GLDebugMessageCallbackI delegate) {
        this.delegate = delegate;
    }

    public static GLDebugMessageCallback create(GLDebugMessageCallbackI callback) {
        return new GLDebugMessageCallback(callback);
    }

    public static String getMessage(int length, long message) {
        return MemoryUtil.memUTF8(message);
    }

    @Override
    public void invoke(int source, int type, int id, int severity, int length, long message, long userParam) {
        if (delegate != null) delegate.invoke(source, type, id, severity, length, message, userParam);
    }
}
