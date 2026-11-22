package org.lwjgl.vulkan;

public class VkPhysicalDeviceFeatures extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPhysicalDeviceFeatures(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceFeatures(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static int MULTIDRAWINDIRECT = 0;
    public boolean multiDrawIndirect() { return false; }
    public static int DRAWINDIRECTFIRSTINSTANCE = 0;
    public boolean drawIndirectFirstInstance() { return false; }
    public static int SAMPLERANISOTROPY = 0;
    public boolean samplerAnisotropy() { return false; }
    public static int FILLMODENONSOLID = 0;
    public boolean fillModeNonSolid() { return false; }
}
