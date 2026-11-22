package org.lwjgl.vulkan;

public class VkClearValue extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkClearValue(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkClearValue(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkClearValue calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkClearValue(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkClearColorValue color() { return null; }
    public org.lwjgl.vulkan.VkClearValue color(org.lwjgl.vulkan.VkClearColorValue p0) { return this; }
    public org.lwjgl.vulkan.VkClearDepthStencilValue depthStencil() { return null; }
    public org.lwjgl.vulkan.VkClearValue depthStencil(org.lwjgl.vulkan.VkClearDepthStencilValue p0) { return this; }
}
