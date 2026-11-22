package org.lwjgl.stb;

import org.lwjgl.system.Callback;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

public abstract class STBIWriteCallback extends Callback {
    protected STBIWriteCallback() {
        super();
    }

    public static ByteBuffer getData(long data, int size) {
        return MemoryUtil.memByteBuffer(data, size);
    }

    public abstract void invoke(long context, long data, int size);
}
