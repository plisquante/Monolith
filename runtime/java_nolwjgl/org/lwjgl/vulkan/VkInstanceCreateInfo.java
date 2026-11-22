package org.lwjgl.vulkan;

public class VkInstanceCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkInstanceCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkInstanceCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkInstanceCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkInstanceCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkInstanceCreateInfo flags(int p0) { return this; }
    public org.lwjgl.vulkan.VkInstanceCreateInfo pApplicationInfo(org.lwjgl.vulkan.VkApplicationInfo p0) { return this; }
    public org.lwjgl.vulkan.VkInstanceCreateInfo pNext(org.lwjgl.vulkan.VkDebugUtilsMessengerCreateInfoEXT p0) { return this; }
    public org.lwjgl.vulkan.VkInstanceCreateInfo ppEnabledExtensionNames(org.lwjgl.PointerBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkInstanceCreateInfo ppEnabledLayerNames(org.lwjgl.PointerBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkInstanceCreateInfo sType$Default() { return this; }
}
