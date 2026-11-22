package org.lwjgl.vulkan;

public class VkApplicationInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkApplicationInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkApplicationInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkApplicationInfo apiVersion(int p0) { return this; }
    public org.lwjgl.vulkan.VkApplicationInfo applicationVersion(int p0) { return this; }
    public static org.lwjgl.vulkan.VkApplicationInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkApplicationInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkApplicationInfo engineVersion(int p0) { return this; }
    public org.lwjgl.vulkan.VkApplicationInfo pApplicationName(java.nio.ByteBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkApplicationInfo pEngineName(java.nio.ByteBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkApplicationInfo sType$Default() { return this; }
}
