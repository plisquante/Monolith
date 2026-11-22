package org.lwjgl.vulkan;

public class VkPhysicalDeviceVulkan11Properties extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceVulkan11Properties(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceVulkan11Properties(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceVulkan11Properties calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceVulkan11Properties(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public void free() {  }
    public long maxMemoryAllocationSize() { return 0L; }
    public org.lwjgl.vulkan.VkPhysicalDeviceVulkan11Properties sType$Default() { return this; }
}
