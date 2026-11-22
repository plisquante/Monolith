package org.lwjgl.vulkan;

public class VkPhysicalDeviceLimits extends org.lwjgl.system.Struct {
    public VkPhysicalDeviceLimits(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceLimits(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public int maxColorAttachments() { return 0; }
    public int maxDrawIndirectCount() { return 0; }
    public int maxImageDimension2D() { return 0; }
    public float maxSamplerAnisotropy() { return 0; }
    public long minUniformBufferOffsetAlignment() { return 0L; }
    public float timestampPeriod() { return 0; }
}
