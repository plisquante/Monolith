package org.lwjgl.vulkan;

public class VkClearRect extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkClearRect(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkClearRect(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkClearRect.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkClearRect get(int index) { return new VkClearRect(address + (long) index * 64L, container); }
        public org.lwjgl.vulkan.VkClearRect.Buffer baseArrayLayer(int p0) { return this; }
        public org.lwjgl.vulkan.VkClearRect.Buffer layerCount(int p0) { return this; }
        public org.lwjgl.vulkan.VkRect2D rect() { return null; }
    }
}
