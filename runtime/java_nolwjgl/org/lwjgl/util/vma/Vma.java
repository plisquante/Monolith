package org.lwjgl.util.vma;

import org.lwjgl.PointerBuffer;
import org.lwjgl.vulkan.VkBufferCreateInfo;
import org.lwjgl.vulkan.VkImageCreateInfo;

import java.nio.LongBuffer;

public final class Vma {
    private Vma() {}

    public static int vmaCreateAllocator(VmaAllocatorCreateInfo pCreateInfo, PointerBuffer pAllocator) { return -1; }
    public static void vmaDestroyAllocator(long allocator) {}
    public static int vmaCreateBuffer(long allocator, VkBufferCreateInfo pBufferCreateInfo, VmaAllocationCreateInfo pAllocationCreateInfo, LongBuffer pBuffer, PointerBuffer pAllocation, VmaAllocationInfo pAllocationInfo) { return -1; }
    public static void vmaDestroyBuffer(long allocator, long buffer, long allocation) {}
    public static int vmaCreateImage(long allocator, VkImageCreateInfo pImageCreateInfo, VmaAllocationCreateInfo pAllocationCreateInfo, LongBuffer pImage, PointerBuffer pAllocation, VmaAllocationInfo pAllocationInfo) { return -1; }
    public static void vmaDestroyImage(long allocator, long image, long allocation) {}
    public static int vmaMapMemory(long allocator, long allocation, PointerBuffer ppData) { return -1; }
    public static void vmaUnmapMemory(long allocator, long allocation) {}
}
