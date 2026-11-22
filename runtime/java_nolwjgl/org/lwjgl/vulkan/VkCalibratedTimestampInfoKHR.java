package org.lwjgl.vulkan;

public class VkCalibratedTimestampInfoKHR extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkCalibratedTimestampInfoKHR(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkCalibratedTimestampInfoKHR(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkCalibratedTimestampInfoKHR sType$Default() { return this; }
    public org.lwjgl.vulkan.VkCalibratedTimestampInfoKHR timeDomain(int p0) { return this; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkCalibratedTimestampInfoKHR get(int index) { return new VkCalibratedTimestampInfoKHR(address + (long) index * 64L, container); }
    }
}
