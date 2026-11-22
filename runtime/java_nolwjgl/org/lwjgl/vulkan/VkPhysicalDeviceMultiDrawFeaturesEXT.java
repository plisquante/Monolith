package org.lwjgl.vulkan;

public class VkPhysicalDeviceMultiDrawFeaturesEXT extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceMultiDrawFeaturesEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceMultiDrawFeaturesEXT(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static int PNEXT = 0;
    public static int STYPE = 0;
    public static int MULTIDRAW = 0;
    public boolean multiDraw() { return false; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceMultiDrawFeaturesEXT calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceMultiDrawFeaturesEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public static org.lwjgl.vulkan.VkPhysicalDeviceMultiDrawFeaturesEXT calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceMultiDrawFeaturesEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPhysicalDeviceMultiDrawFeaturesEXT sType$Default() { return this; }
    public long pNext() { return 0L; }
    public void free() {  }
}
