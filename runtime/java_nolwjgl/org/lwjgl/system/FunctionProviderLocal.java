package org.lwjgl.system;

import java.nio.ByteBuffer;

public interface FunctionProviderLocal extends FunctionProvider {
    default long getFunctionAddress(long handle, ByteBuffer functionName) {
        return getFunctionAddress(functionName == null ? "" : MemoryUtil.memUTF8(MemoryUtil.memAddress(functionName)));
    }
}
