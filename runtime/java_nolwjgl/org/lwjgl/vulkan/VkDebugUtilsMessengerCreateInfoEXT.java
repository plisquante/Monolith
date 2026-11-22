package org.lwjgl.vulkan;

public class VkDebugUtilsMessengerCreateInfoEXT extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkDebugUtilsMessengerCreateInfoEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkDebugUtilsMessengerCreateInfoEXT(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkDebugUtilsMessengerCreateInfoEXT calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkDebugUtilsMessengerCreateInfoEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkDebugUtilsMessengerCreateInfoEXT messageSeverity(int p0) { return this; }
    public org.lwjgl.vulkan.VkDebugUtilsMessengerCreateInfoEXT messageType(int p0) { return this; }
    public org.lwjgl.vulkan.VkDebugUtilsMessengerCreateInfoEXT pfnUserCallback(org.lwjgl.vulkan.VkDebugUtilsMessengerCallbackEXTI p0) { return this; }
    public org.lwjgl.vulkan.VkDebugUtilsMessengerCreateInfoEXT sType$Default() { return this; }
}
