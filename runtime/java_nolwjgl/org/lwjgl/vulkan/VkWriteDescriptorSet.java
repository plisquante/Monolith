package org.lwjgl.vulkan;

public class VkWriteDescriptorSet extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkWriteDescriptorSet(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkWriteDescriptorSet(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkWriteDescriptorSet.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public org.lwjgl.vulkan.VkWriteDescriptorSet descriptorCount(int p0) { return this; }
    public org.lwjgl.vulkan.VkWriteDescriptorSet descriptorType(int p0) { return this; }
    public org.lwjgl.vulkan.VkWriteDescriptorSet dstArrayElement(int p0) { return this; }
    public org.lwjgl.vulkan.VkWriteDescriptorSet dstBinding(int p0) { return this; }
    public org.lwjgl.vulkan.VkWriteDescriptorSet pBufferInfo(org.lwjgl.vulkan.VkDescriptorBufferInfo.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkWriteDescriptorSet pImageInfo(org.lwjgl.vulkan.VkDescriptorImageInfo.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkWriteDescriptorSet pTexelBufferView(java.nio.LongBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkWriteDescriptorSet sType$Default() { return this; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkWriteDescriptorSet get(int index) { return new VkWriteDescriptorSet(address + (long) index * 64L, container); }
        public org.lwjgl.system.CustomBuffer flip() { return null; }
    }
}
