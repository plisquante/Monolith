package org.lwjgl.vulkan;

public class VkDebugUtilsObjectNameInfoEXT extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkDebugUtilsObjectNameInfoEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkDebugUtilsObjectNameInfoEXT(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkDebugUtilsObjectNameInfoEXT calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkDebugUtilsObjectNameInfoEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkDebugUtilsObjectNameInfoEXT objectHandle(long p0) { return this; }
    public org.lwjgl.vulkan.VkDebugUtilsObjectNameInfoEXT objectType(int p0) { return this; }
    public org.lwjgl.vulkan.VkDebugUtilsObjectNameInfoEXT pObjectName(java.nio.ByteBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkDebugUtilsObjectNameInfoEXT sType$Default() { return this; }
}
