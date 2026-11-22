package org.lwjgl.vulkan;

public class VkPhysicalDeviceFeatures2 extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public static int FEATURES = 0;
    public VkPhysicalDeviceFeatures2(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceFeatures2(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceFeatures2 calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceFeatures2(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public static org.lwjgl.vulkan.VkPhysicalDeviceFeatures2 calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceFeatures2(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPhysicalDeviceFeatures features() { return null; }
    public void free() {  }
    public long pNext() { return 0L; }
    public org.lwjgl.vulkan.VkPhysicalDeviceFeatures2 sType$Default() { return this; }
    public static int PNEXT = 0;
    public static int STYPE = 0;
}
