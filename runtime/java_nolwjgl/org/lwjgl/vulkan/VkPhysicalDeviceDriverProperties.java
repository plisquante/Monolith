package org.lwjgl.vulkan;

public class VkPhysicalDeviceDriverProperties extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceDriverProperties(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceDriverProperties(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceDriverProperties calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceDriverProperties(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public int driverID() { return 0; }
    public java.lang.String driverInfoString() { return ""; }
    public java.lang.String driverNameString() { return ""; }
    public void free() {  }
    public org.lwjgl.vulkan.VkPhysicalDeviceDriverProperties sType$Default() { return this; }
}
