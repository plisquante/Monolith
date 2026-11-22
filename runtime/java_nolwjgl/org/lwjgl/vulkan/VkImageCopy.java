package org.lwjgl.vulkan;

public class VkImageCopy extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkImageCopy(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkImageCopy(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkImageCopy.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkImageCopy get(int index) { return new VkImageCopy(address + (long) index * 64L, container); }
        public org.lwjgl.vulkan.VkOffset3D dstOffset() { return null; }
        public org.lwjgl.vulkan.VkImageCopy.Buffer dstSubresource(org.lwjgl.vulkan.VkImageSubresourceLayers p0) { return this; }
        public org.lwjgl.vulkan.VkExtent3D extent() { return null; }
        public org.lwjgl.vulkan.VkOffset3D srcOffset() { return null; }
        public org.lwjgl.vulkan.VkImageCopy.Buffer srcSubresource(org.lwjgl.vulkan.VkImageSubresourceLayers p0) { return this; }
    }
}
