package org.lwjgl.vulkan;

public class VkRenderingAttachmentInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkRenderingAttachmentInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkRenderingAttachmentInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkRenderingAttachmentInfo.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public static org.lwjgl.vulkan.VkRenderingAttachmentInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkRenderingAttachmentInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkRenderingAttachmentInfo clearValue(org.lwjgl.vulkan.VkClearValue p0) { return this; }
    public org.lwjgl.vulkan.VkRenderingAttachmentInfo imageLayout(int p0) { return this; }
    public org.lwjgl.vulkan.VkRenderingAttachmentInfo imageView(long p0) { return this; }
    public org.lwjgl.vulkan.VkRenderingAttachmentInfo loadOp(int p0) { return this; }
    public org.lwjgl.vulkan.VkRenderingAttachmentInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkRenderingAttachmentInfo storeOp(int p0) { return this; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkRenderingAttachmentInfo get(int index) { return new VkRenderingAttachmentInfo(address + (long) index * 64L, container); }
        public org.lwjgl.vulkan.VkRenderingAttachmentInfo.Buffer clearValue(org.lwjgl.vulkan.VkClearValue p0) { return this; }
        public org.lwjgl.vulkan.VkRenderingAttachmentInfo.Buffer imageLayout(int p0) { return this; }
        public org.lwjgl.vulkan.VkRenderingAttachmentInfo.Buffer imageView(long p0) { return this; }
        public org.lwjgl.vulkan.VkRenderingAttachmentInfo.Buffer loadOp(int p0) { return this; }
        public org.lwjgl.system.CustomBuffer position(int p0) { return null; }
        public org.lwjgl.vulkan.VkRenderingAttachmentInfo.Buffer sType$Default() { return this; }
        public org.lwjgl.vulkan.VkRenderingAttachmentInfo.Buffer storeOp(int p0) { return this; }
    }
}
