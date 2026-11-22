package org.lwjgl.vulkan;

public class VkDrawIndirectCommand extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public static int SIZEOF = 0;
    public VkDrawIndirectCommand(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkDrawIndirectCommand(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
}
