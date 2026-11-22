package org.lwjgl.util.vma;

import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;
import org.lwjgl.vulkan.VkDevice;
import org.lwjgl.vulkan.VkInstance;

import java.nio.ByteBuffer;

public class VmaVulkanFunctions extends Struct<VmaVulkanFunctions> {
    public static final int SIZEOF = 256;

    public VmaVulkanFunctions(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static VmaVulkanFunctions calloc(MemoryStack stack) {
        ByteBuffer b = stack.calloc(8, SIZEOF);
        return new VmaVulkanFunctions(MemoryUtil.memAddress(b), b);
    }

    public VmaVulkanFunctions set(VkInstance instance, VkDevice device) { return this; }
}
