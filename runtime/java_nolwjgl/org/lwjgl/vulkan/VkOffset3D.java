package org.lwjgl.vulkan;

public class VkOffset3D extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkOffset3D(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkOffset3D(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkOffset3D.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public org.lwjgl.vulkan.VkOffset3D set(int p0, int p1, int p2) { return this; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkOffset3D get(int index) { return new VkOffset3D(address + (long) index * 64L, container); }
        public org.lwjgl.system.CustomBuffer position(int p0) { return null; }
        public org.lwjgl.vulkan.VkOffset3D.Buffer x(int p0) { return this; }
        public org.lwjgl.vulkan.VkOffset3D.Buffer y(int p0) { return this; }
        public org.lwjgl.vulkan.VkOffset3D.Buffer z(int p0) { return this; }
    }
}
