package org.lwjgl.vulkan;

public class VkMemoryBarrier extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkMemoryBarrier(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkMemoryBarrier(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkMemoryBarrier get(int index) { return new VkMemoryBarrier(address + (long) index * 64L, container); }
    }
}
