package org.lwjgl.util.spvc;

import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;
import org.lwjgl.system.StructBuffer;

import java.nio.ByteBuffer;

public class SpvcReflectedResource extends Struct<SpvcReflectedResource> {
    public static final int SIZEOF = 24;

    public SpvcReflectedResource(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public int id() { return MemoryUtil.memGetInt(address); }
    public int base_type_id() { return MemoryUtil.memGetInt(address + 4); }
    public int type_id() { return MemoryUtil.memGetInt(address + 8); }
    /** {@code const char *name} of spvc_reflected_resource (after three ids and padding). */
    public String nameString() {
        long p = MemoryUtil.memGetAddress(address + 16);
        return p == 0L ? "" : MemoryUtil.memUTF8(p);
    }

    public static Buffer create(long address, int capacity) {
        return new Buffer(address, capacity);
    }

    public static class Buffer extends StructBuffer<SpvcReflectedResource, Buffer> {
        public Buffer(long address, int capacity) {
            super(address, null, -1, 0, capacity, capacity);
        }

        @Override
        public int sizeof() { return SIZEOF; }

        @Override
        public SpvcReflectedResource get(int index) {
            return new SpvcReflectedResource(address + (long) index * SIZEOF, container);
        }
    }
}
