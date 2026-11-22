package org.lwjgl.vulkan;

public class VkSurfaceFormatKHR extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkSurfaceFormatKHR(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkSurfaceFormatKHR(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkSurfaceFormatKHR.Buffer calloc(int p0) { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(Math.max(1, p0) * 64); return new Buffer(b); }
    public int colorSpace() { return 0; }
    public int format() { return 0; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkSurfaceFormatKHR get(int index) { return new VkSurfaceFormatKHR(address + (long) index * 64L, container); }
        public java.util.Iterator iterator() { return null; }
    }
}
