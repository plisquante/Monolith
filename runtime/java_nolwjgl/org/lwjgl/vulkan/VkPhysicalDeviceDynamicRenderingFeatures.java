package org.lwjgl.vulkan;

public class VkPhysicalDeviceDynamicRenderingFeatures extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceDynamicRenderingFeatures(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceDynamicRenderingFeatures(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static int PNEXT = 0;
    public static int STYPE = 0;
    public static int DYNAMICRENDERING = 0;
    public boolean dynamicRendering() { return false; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceDynamicRenderingFeatures calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceDynamicRenderingFeatures(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public static org.lwjgl.vulkan.VkPhysicalDeviceDynamicRenderingFeatures calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceDynamicRenderingFeatures(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPhysicalDeviceDynamicRenderingFeatures sType$Default() { return this; }
    public long pNext() { return 0L; }
    public void free() {  }
}
