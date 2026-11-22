package org.lwjgl.vulkan;

public class VkPhysicalDeviceVulkan11Features extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceVulkan11Features(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceVulkan11Features(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static int PNEXT = 0;
    public static int STYPE = 0;
    public static int SHADERDRAWPARAMETERS = 0;
    public boolean shaderDrawParameters() { return false; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceVulkan11Features calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceVulkan11Features(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public static org.lwjgl.vulkan.VkPhysicalDeviceVulkan11Features calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceVulkan11Features(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPhysicalDeviceVulkan11Features sType$Default() { return this; }
    public long pNext() { return 0L; }
    public void free() {  }
}
