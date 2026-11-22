package org.lwjgl.vulkan;

public class VkPhysicalDevicePortabilitySubsetFeaturesKHR extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDevicePortabilitySubsetFeaturesKHR(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDevicePortabilitySubsetFeaturesKHR(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static int PNEXT = 0;
    public static int STYPE = 0;
    public static int TRIANGLEFANS = 0;
    public boolean triangleFans() { return false; }
    public static org.lwjgl.vulkan.VkPhysicalDevicePortabilitySubsetFeaturesKHR calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDevicePortabilitySubsetFeaturesKHR(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public static org.lwjgl.vulkan.VkPhysicalDevicePortabilitySubsetFeaturesKHR calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDevicePortabilitySubsetFeaturesKHR(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPhysicalDevicePortabilitySubsetFeaturesKHR sType$Default() { return this; }
    public long pNext() { return 0L; }
    public void free() {  }
}
