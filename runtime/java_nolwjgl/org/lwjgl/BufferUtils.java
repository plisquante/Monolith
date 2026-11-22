package org.lwjgl;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.DoubleBuffer;
import java.nio.FloatBuffer;
import java.nio.IntBuffer;
import java.nio.LongBuffer;
import java.nio.ShortBuffer;

public final class BufferUtils {
    private BufferUtils() {}

    public static ByteBuffer createByteBuffer(int capacity) {
        return ByteBuffer.allocateDirect(capacity).order(ByteOrder.nativeOrder());
    }

    public static ShortBuffer createShortBuffer(int capacity) {
        return createByteBuffer(capacity << 1).asShortBuffer();
    }

    public static IntBuffer createIntBuffer(int capacity) {
        return createByteBuffer(capacity << 2).asIntBuffer();
    }

    public static LongBuffer createLongBuffer(int capacity) {
        return createByteBuffer(capacity << 3).asLongBuffer();
    }

    public static FloatBuffer createFloatBuffer(int capacity) {
        return createByteBuffer(capacity << 2).asFloatBuffer();
    }

    public static DoubleBuffer createDoubleBuffer(int capacity) {
        return createByteBuffer(capacity << 3).asDoubleBuffer();
    }
}
