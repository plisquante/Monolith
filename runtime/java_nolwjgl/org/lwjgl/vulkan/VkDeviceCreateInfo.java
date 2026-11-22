package org.lwjgl.vulkan;

public class VkDeviceCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkDeviceCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkDeviceCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkDeviceCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkDeviceCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkDeviceCreateInfo pEnabledFeatures(org.lwjgl.vulkan.VkPhysicalDeviceFeatures p0) { return this; }
    public org.lwjgl.vulkan.VkDeviceCreateInfo pNext(long p0) { return this; }
    public org.lwjgl.vulkan.VkDeviceCreateInfo pQueueCreateInfos(org.lwjgl.vulkan.VkDeviceQueueCreateInfo.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkDeviceCreateInfo ppEnabledExtensionNames(org.lwjgl.PointerBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkDeviceCreateInfo sType$Default() { return this; }
}
