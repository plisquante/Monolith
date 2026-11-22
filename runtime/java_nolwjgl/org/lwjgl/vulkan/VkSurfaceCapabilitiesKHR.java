package org.lwjgl.vulkan;

public class VkSurfaceCapabilitiesKHR extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkSurfaceCapabilitiesKHR(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkSurfaceCapabilitiesKHR(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkSurfaceCapabilitiesKHR calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkSurfaceCapabilitiesKHR(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public int currentTransform() { return 0; }
    public org.lwjgl.vulkan.VkExtent2D maxImageExtent() { return null; }
    public int minImageCount() { return 0; }
    public org.lwjgl.vulkan.VkExtent2D minImageExtent() { return null; }
}
