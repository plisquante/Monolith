package org.lwjgl.vulkan;

public class VkVertexInputBindingDivisorDescriptionEXT extends org.lwjgl.vulkan.VkVertexInputBindingDivisorDescription {
    public VkVertexInputBindingDivisorDescriptionEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkVertexInputBindingDivisorDescriptionEXT(java.nio.ByteBuffer container) { super(container); }
    public org.lwjgl.vulkan.VkVertexInputBindingDivisorDescriptionEXT binding(int p0) { return this; }
    public static org.lwjgl.vulkan.VkVertexInputBindingDivisorDescriptionEXT.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public static org.lwjgl.vulkan.VkVertexInputBindingDivisorDescriptionEXT calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkVertexInputBindingDivisorDescriptionEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkVertexInputBindingDivisorDescriptionEXT divisor(int p0) { return this; }

    public static class Buffer extends org.lwjgl.vulkan.VkVertexInputBindingDivisorDescription.Buffer {
        public Buffer(long address, int capacity) { super(address, capacity); }
        public Buffer(java.nio.ByteBuffer container) { super(container); }
        public org.lwjgl.system.CustomBuffer flip() { return null; }
        public org.lwjgl.system.StructBuffer put(org.lwjgl.system.Struct p0) { return null; }
    }
}
