package org.lwjgl.vulkan;

public class VkPhysicalDeviceSynchronization2Features extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceSynchronization2Features(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceSynchronization2Features(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static int PNEXT = 0;
    public static int STYPE = 0;
    public static int SYNCHRONIZATION2 = 0;
    public boolean synchronization2() { return false; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceSynchronization2Features calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceSynchronization2Features(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public static org.lwjgl.vulkan.VkPhysicalDeviceSynchronization2Features calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceSynchronization2Features(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPhysicalDeviceSynchronization2Features sType$Default() { return this; }
    public long pNext() { return 0L; }
    public void free() {  }
}
