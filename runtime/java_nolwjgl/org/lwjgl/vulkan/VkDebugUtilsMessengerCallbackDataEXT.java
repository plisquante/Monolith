package org.lwjgl.vulkan;

public class VkDebugUtilsMessengerCallbackDataEXT extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkDebugUtilsMessengerCallbackDataEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkDebugUtilsMessengerCallbackDataEXT(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkDebugUtilsMessengerCallbackDataEXT create(long p0) { return p0 == 0L ? null : new VkDebugUtilsMessengerCallbackDataEXT(p0, null); }
    public java.lang.String pMessageString() { return ""; }
}
