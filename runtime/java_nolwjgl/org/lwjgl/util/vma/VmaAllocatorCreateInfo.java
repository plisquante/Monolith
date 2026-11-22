package org.lwjgl.util.vma;

import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;
import org.lwjgl.vulkan.VkDevice;
import org.lwjgl.vulkan.VkInstance;
import org.lwjgl.vulkan.VkPhysicalDevice;

import java.nio.ByteBuffer;

public class VmaAllocatorCreateInfo extends Struct<VmaAllocatorCreateInfo> {
    public static final int SIZEOF = 80;

    public VmaAllocatorCreateInfo(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static VmaAllocatorCreateInfo calloc(MemoryStack stack) {
        ByteBuffer b = stack.calloc(8, SIZEOF);
        return new VmaAllocatorCreateInfo(MemoryUtil.memAddress(b), b);
    }

    public VmaAllocatorCreateInfo device(VkDevice value) { return this; }
    public VmaAllocatorCreateInfo instance(VkInstance value) { return this; }
    public VmaAllocatorCreateInfo physicalDevice(VkPhysicalDevice value) { return this; }
    public VmaAllocatorCreateInfo pVulkanFunctions(VmaVulkanFunctions value) { return this; }
    public VmaAllocatorCreateInfo vulkanApiVersion(int value) { return this; }
}
