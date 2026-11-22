package org.lwjgl.vulkan;

public class VkBufferImageCopy extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkBufferImageCopy(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkBufferImageCopy(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkBufferImageCopy.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkBufferImageCopy get(int index) { return new VkBufferImageCopy(address + (long) index * 64L, container); }
        public org.lwjgl.vulkan.VkBufferImageCopy.Buffer bufferImageHeight(int p0) { return this; }
        public org.lwjgl.vulkan.VkBufferImageCopy.Buffer bufferOffset(long p0) { return this; }
        public org.lwjgl.vulkan.VkBufferImageCopy.Buffer bufferRowLength(int p0) { return this; }
        public org.lwjgl.vulkan.VkExtent3D imageExtent() { return null; }
        public org.lwjgl.vulkan.VkOffset3D imageOffset() { return null; }
        public org.lwjgl.vulkan.VkImageSubresourceLayers imageSubresource() { return null; }
    }
}
