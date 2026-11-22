package org.lwjgl.vulkan;

public class VkPhysicalDeviceProperties extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceProperties(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceProperties(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public int apiVersion() { return 0; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceProperties calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceProperties(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public int deviceID() { return 0; }
    public java.lang.String deviceNameString() { return ""; }
    public int deviceType() { return 0; }
    public org.lwjgl.vulkan.VkPhysicalDeviceLimits limits() { return null; }
    public int vendorID() { return 0; }
}
