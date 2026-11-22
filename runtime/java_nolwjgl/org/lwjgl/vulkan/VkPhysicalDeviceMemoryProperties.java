package org.lwjgl.vulkan;

public class VkPhysicalDeviceMemoryProperties extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceMemoryProperties(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceMemoryProperties(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceMemoryProperties calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceMemoryProperties(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public int memoryHeapCount() { return 0; }
    public org.lwjgl.vulkan.VkMemoryHeap memoryHeaps(int p0) { return null; }
    public int memoryTypeCount() { return 0; }
    public org.lwjgl.vulkan.VkMemoryType memoryTypes(int p0) { return null; }
}
