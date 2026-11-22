package org.lwjgl.vulkan;

public class VkExtensionProperties extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkExtensionProperties(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkExtensionProperties(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkExtensionProperties.Buffer calloc(int p0) { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(Math.max(1, p0) * 64); return new Buffer(b); }
    public static org.lwjgl.vulkan.VkExtensionProperties.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }
    public java.lang.String extensionNameString() { return ""; }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkExtensionProperties get(int index) { return new VkExtensionProperties(address + (long) index * 64L, container); }
        public void close() {  }
        public java.lang.String extensionNameString() { return ""; }
        public void free() {  }
        public java.util.Iterator iterator() { return null; }
        public int limit() { return 0; }
        public org.lwjgl.system.CustomBuffer limit(int p0) { return null; }
        public org.lwjgl.system.CustomBuffer position(int p0) { return null; }
        public java.util.stream.Stream stream() { return null; }
    }
}
