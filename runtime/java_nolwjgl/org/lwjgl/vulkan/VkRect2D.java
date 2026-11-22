package org.lwjgl.vulkan;

public class VkRect2D extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkRect2D(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkRect2D(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkRect2D.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public static org.lwjgl.vulkan.VkRect2D calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkRect2D(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkExtent2D extent() { return null; }
    public org.lwjgl.vulkan.VkOffset2D offset() { return null; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkRect2D get(int index) { return new VkRect2D(address + (long) index * 64L, container); }
        public org.lwjgl.vulkan.VkExtent2D extent() { return null; }
        public org.lwjgl.vulkan.VkOffset2D offset() { return null; }
    }
}
