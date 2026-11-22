package org.lwjgl.vulkan;

public class VkDebugUtilsLabelEXT extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkDebugUtilsLabelEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkDebugUtilsLabelEXT(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkDebugUtilsLabelEXT calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkDebugUtilsLabelEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkDebugUtilsLabelEXT pLabelName(java.nio.ByteBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkDebugUtilsLabelEXT sType$Default() { return this; }
}
