package org.lwjgl.vulkan;

public class VkVertexInputAttributeDescription extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkVertexInputAttributeDescription(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkVertexInputAttributeDescription(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkVertexInputAttributeDescription binding(int p0) { return this; }
    public static org.lwjgl.vulkan.VkVertexInputAttributeDescription.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public static org.lwjgl.vulkan.VkVertexInputAttributeDescription calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkVertexInputAttributeDescription(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkVertexInputAttributeDescription format(int p0) { return this; }
    public org.lwjgl.vulkan.VkVertexInputAttributeDescription location(int p0) { return this; }
    public org.lwjgl.vulkan.VkVertexInputAttributeDescription offset(int p0) { return this; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkVertexInputAttributeDescription get(int index) { return new VkVertexInputAttributeDescription(address + (long) index * 64L, container); }
        public org.lwjgl.system.CustomBuffer flip() { return null; }
        public org.lwjgl.system.StructBuffer put(org.lwjgl.system.Struct p0) { return null; }
    }
}
