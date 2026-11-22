package org.lwjgl.vulkan;

public class VkImageMemoryBarrier2 extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkImageMemoryBarrier2(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkImageMemoryBarrier2(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkImageMemoryBarrier2 get(int index) { return new VkImageMemoryBarrier2(address + (long) index * 64L, container); }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer dstAccessMask(long p0) { return this; }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer dstQueueFamilyIndex(int p0) { return this; }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer dstStageMask(long p0) { return this; }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer image(long p0) { return this; }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer newLayout(int p0) { return this; }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer oldLayout(int p0) { return this; }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer sType$Default() { return this; }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer srcAccessMask(long p0) { return this; }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer srcQueueFamilyIndex(int p0) { return this; }
        public org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer srcStageMask(long p0) { return this; }
        public org.lwjgl.vulkan.VkImageSubresourceRange subresourceRange() { return null; }
    }
}
