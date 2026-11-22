package org.lwjgl.vulkan;

public class VkLayerProperties extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkLayerProperties(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkLayerProperties(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkLayerProperties.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public java.lang.String layerNameString() { return ""; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkLayerProperties get(int index) { return new VkLayerProperties(address + (long) index * 64L, container); }
    }
}
