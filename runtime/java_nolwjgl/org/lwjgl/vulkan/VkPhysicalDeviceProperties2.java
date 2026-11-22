package org.lwjgl.vulkan;

public class VkPhysicalDeviceProperties2 extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public static int PNEXT = 0;
    public static int STYPE = 0;
    public VkPhysicalDeviceProperties2(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceProperties2(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceProperties2 calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceProperties2(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public static org.lwjgl.vulkan.VkPhysicalDeviceProperties2 calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceProperties2(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public void free() {  }
    public static long npNext(long p0) { return 0L; }
    public static void npNext(long p0, long p1) {  }
    public static int nsType(long p0) { return 0; }
    public static void nsType(long p0, int p1) {  }
    public org.lwjgl.vulkan.VkPhysicalDeviceProperties2 pNext(org.lwjgl.vulkan.VkPhysicalDeviceDriverProperties p0) { return this; }
    public org.lwjgl.vulkan.VkPhysicalDeviceProperties2 pNext(org.lwjgl.vulkan.VkPhysicalDeviceMultiDrawPropertiesEXT p0) { return this; }
    public org.lwjgl.vulkan.VkPhysicalDeviceProperties2 pNext(org.lwjgl.vulkan.VkPhysicalDeviceVulkan11Properties p0) { return this; }
    public org.lwjgl.vulkan.VkPhysicalDeviceProperties properties() { return null; }
    public org.lwjgl.vulkan.VkPhysicalDeviceProperties2 sType$Default() { return this; }
}
