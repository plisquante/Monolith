package org.lwjgl.vulkan;

public class VkSwapchainCreateInfoKHR extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkSwapchainCreateInfoKHR(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkSwapchainCreateInfoKHR(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkSwapchainCreateInfoKHR calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkSwapchainCreateInfoKHR(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR clipped(boolean p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR compositeAlpha(int p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR imageArrayLayers(int p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR imageColorSpace(int p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR imageExtent(org.lwjgl.vulkan.VkExtent2D p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR imageFormat(int p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR imageSharingMode(int p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR imageUsage(int p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR minImageCount(int p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR preTransform(int p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR presentMode(int p0) { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR sType$Default() { return this; }
    public org.lwjgl.vulkan.VkSwapchainCreateInfoKHR surface(long p0) { return this; }
}
