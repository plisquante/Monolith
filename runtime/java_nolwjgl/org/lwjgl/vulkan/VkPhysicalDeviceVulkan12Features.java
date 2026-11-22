package org.lwjgl.vulkan;

public class VkPhysicalDeviceVulkan12Features extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceVulkan12Features(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceVulkan12Features(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static int PNEXT = 0;
    public static int STYPE = 0;
    public static int TIMELINESEMAPHORE = 0;
    public boolean timelineSemaphore() { return false; }
    public static int HOSTQUERYRESET = 0;
    public boolean hostQueryReset() { return false; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceVulkan12Features calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceVulkan12Features(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public static org.lwjgl.vulkan.VkPhysicalDeviceVulkan12Features calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceVulkan12Features(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPhysicalDeviceVulkan12Features sType$Default() { return this; }
    public long pNext() { return 0L; }
    public void free() {  }
}
