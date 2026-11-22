package org.lwjgl.vulkan;

public class VkCheckpointData2NV extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkCheckpointData2NV(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkCheckpointData2NV(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkCheckpointData2NV.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public long pCheckpointMarker() { return 0L; }
    public org.lwjgl.vulkan.VkCheckpointData2NV sType$Default() { return this; }
    public long stage() { return 0L; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkCheckpointData2NV get(int index) { return new VkCheckpointData2NV(address + (long) index * 64L, container); }
        public int remaining() { return 0; }
    }
}
