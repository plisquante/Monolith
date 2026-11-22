package org.lwjgl.vulkan;

public class VkDescriptorSetLayoutBinding extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkDescriptorSetLayoutBinding(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkDescriptorSetLayoutBinding(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkDescriptorSetLayoutBinding binding(int p0) { return this; }
    public static org.lwjgl.vulkan.VkDescriptorSetLayoutBinding.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public static org.lwjgl.vulkan.VkDescriptorSetLayoutBinding calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkDescriptorSetLayoutBinding(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkDescriptorSetLayoutBinding descriptorCount(int p0) { return this; }
    public org.lwjgl.vulkan.VkDescriptorSetLayoutBinding descriptorType(int p0) { return this; }
    public org.lwjgl.vulkan.VkDescriptorSetLayoutBinding stageFlags(int p0) { return this; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkDescriptorSetLayoutBinding get(int index) { return new VkDescriptorSetLayoutBinding(address + (long) index * 64L, container); }
        public org.lwjgl.system.CustomBuffer flip() { return null; }
        public org.lwjgl.system.StructBuffer put(org.lwjgl.system.Struct p0) { return null; }
    }
}
