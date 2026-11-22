package org.lwjgl.vulkan;

public class VkCalibratedTimestampInfoEXT extends org.lwjgl.vulkan.VkCalibratedTimestampInfoKHR {
    public VkCalibratedTimestampInfoEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkCalibratedTimestampInfoEXT(java.nio.ByteBuffer container) { super(container); }
    public static org.lwjgl.vulkan.VkCalibratedTimestampInfoEXT.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }

    public static class Buffer extends org.lwjgl.vulkan.VkCalibratedTimestampInfoKHR.Buffer {
        public Buffer(long address, int capacity) { super(address, capacity); }
        public Buffer(java.nio.ByteBuffer container) { super(container); }
        public int capacity() { return 0; }
    }
}
