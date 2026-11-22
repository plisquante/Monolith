package org.lwjgl.vulkan;

public class VkPhysicalDeviceMultiDrawPropertiesEXT extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceMultiDrawPropertiesEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceMultiDrawPropertiesEXT(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceMultiDrawPropertiesEXT calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceMultiDrawPropertiesEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public void free() {  }
    public int maxMultiDrawCount() { return 0; }
    public org.lwjgl.vulkan.VkPhysicalDeviceMultiDrawPropertiesEXT sType$Default() { return this; }
}
