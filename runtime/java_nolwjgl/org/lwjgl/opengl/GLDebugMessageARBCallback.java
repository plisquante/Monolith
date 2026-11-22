package org.lwjgl.opengl;

import org.lwjgl.system.Callback;

public final class GLDebugMessageARBCallback extends Callback implements GLDebugMessageARBCallbackI {
    private final GLDebugMessageARBCallbackI delegate;

    private GLDebugMessageARBCallback(GLDebugMessageARBCallbackI delegate) {
        this.delegate = delegate;
    }

    public static GLDebugMessageARBCallback create(GLDebugMessageARBCallbackI callback) {
        return new GLDebugMessageARBCallback(callback);
    }

    @Override
    public void invoke(int source, int type, int id, int severity, int length, long message, long userParam) {
        if (delegate != null) delegate.invoke(source, type, id, severity, length, message, userParam);
    }
}
