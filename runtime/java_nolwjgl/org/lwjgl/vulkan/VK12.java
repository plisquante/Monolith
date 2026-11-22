package org.lwjgl.vulkan;

public class VK12 extends org.lwjgl.vulkan.VK11 {
    public static int VK_API_VERSION_1_1 = 0;
    public static int VK_API_VERSION_1_2 = 0;
    public static int vkAllocateCommandBuffers(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkCommandBufferAllocateInfo p1, org.lwjgl.PointerBuffer p2) { return 0; }
    public static int vkBeginCommandBuffer(org.lwjgl.vulkan.VkCommandBuffer p0, org.lwjgl.vulkan.VkCommandBufferBeginInfo p1) { return 0; }
    public static void vkCmdBindIndexBuffer(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, long p2, int p3) {  }
    public static void vkCmdBindPipeline(org.lwjgl.vulkan.VkCommandBuffer p0, int p1, long p2) {  }
    public static void vkCmdBindVertexBuffers(org.lwjgl.vulkan.VkCommandBuffer p0, int p1, java.nio.LongBuffer p2, java.nio.LongBuffer p3) {  }
    public static void vkCmdBlitImage(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, int p2, long p3, int p4, org.lwjgl.vulkan.VkImageBlit.Buffer p5, int p6) {  }
    public static void vkCmdClearAttachments(org.lwjgl.vulkan.VkCommandBuffer p0, org.lwjgl.vulkan.VkClearAttachment.Buffer p1, org.lwjgl.vulkan.VkClearRect.Buffer p2) {  }
    public static void vkCmdClearColorImage(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, int p2, org.lwjgl.vulkan.VkClearColorValue p3, org.lwjgl.vulkan.VkImageSubresourceRange p4) {  }
    public static void vkCmdClearDepthStencilImage(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, int p2, org.lwjgl.vulkan.VkClearDepthStencilValue p3, org.lwjgl.vulkan.VkImageSubresourceRange p4) {  }
    public static void vkCmdCopyBuffer(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, long p2, org.lwjgl.vulkan.VkBufferCopy.Buffer p3) {  }
    public static void vkCmdCopyBufferToImage(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, long p2, int p3, org.lwjgl.vulkan.VkBufferImageCopy.Buffer p4) {  }
    public static void vkCmdCopyImage(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, int p2, long p3, int p4, org.lwjgl.vulkan.VkImageCopy.Buffer p5) {  }
    public static void vkCmdCopyImageToBuffer(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, int p2, long p3, org.lwjgl.vulkan.VkBufferImageCopy.Buffer p4) {  }
    public static void vkCmdDraw(org.lwjgl.vulkan.VkCommandBuffer p0, int p1, int p2, int p3, int p4) {  }
    public static void vkCmdDrawIndexed(org.lwjgl.vulkan.VkCommandBuffer p0, int p1, int p2, int p3, int p4, int p5) {  }
    public static void vkCmdDrawIndexedIndirect(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, long p2, int p3, int p4) {  }
    public static void vkCmdDrawIndirect(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, long p2, int p3, int p4) {  }
    public static void vkCmdPipelineBarrier(org.lwjgl.vulkan.VkCommandBuffer p0, int p1, int p2, int p3, org.lwjgl.vulkan.VkMemoryBarrier.Buffer p4, org.lwjgl.vulkan.VkBufferMemoryBarrier.Buffer p5, org.lwjgl.vulkan.VkImageMemoryBarrier.Buffer p6) {  }
    public static void vkCmdPushConstants(org.lwjgl.vulkan.VkCommandBuffer p0, long p1, int p2, int p3, java.nio.ByteBuffer p4) {  }
    public static void vkCmdSetScissor(org.lwjgl.vulkan.VkCommandBuffer p0, int p1, org.lwjgl.vulkan.VkRect2D.Buffer p2) {  }
    public static void vkCmdSetViewport(org.lwjgl.vulkan.VkCommandBuffer p0, int p1, org.lwjgl.vulkan.VkViewport.Buffer p2) {  }
    public static int vkCreateBufferView(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkBufferViewCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, java.nio.LongBuffer p3) { return 0; }
    public static int vkCreateCommandPool(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkCommandPoolCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, java.nio.LongBuffer p3) { return 0; }
    public static int vkCreateDescriptorSetLayout(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkDescriptorSetLayoutCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, java.nio.LongBuffer p3) { return 0; }
    public static int vkCreateDevice(org.lwjgl.vulkan.VkPhysicalDevice p0, org.lwjgl.vulkan.VkDeviceCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, org.lwjgl.PointerBuffer p3) { return 0; }
    public static int vkCreateGraphicsPipelines(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer p2, org.lwjgl.vulkan.VkAllocationCallbacks p3, java.nio.LongBuffer p4) { return 0; }
    public static int vkCreateImageView(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkImageViewCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, java.nio.LongBuffer p3) { return 0; }
    public static int vkCreateInstance(org.lwjgl.vulkan.VkInstanceCreateInfo p0, org.lwjgl.vulkan.VkAllocationCallbacks p1, org.lwjgl.PointerBuffer p2) { return 0; }
    public static int vkCreatePipelineLayout(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkPipelineLayoutCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, java.nio.LongBuffer p3) { return 0; }
    public static int vkCreateQueryPool(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkQueryPoolCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, java.nio.LongBuffer p3) { return 0; }
    public static int vkCreateSampler(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkSamplerCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, java.nio.LongBuffer p3) { return 0; }
    public static int vkCreateSemaphore(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkSemaphoreCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, java.nio.LongBuffer p3) { return 0; }
    public static int vkCreateShaderModule(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkShaderModuleCreateInfo p1, org.lwjgl.vulkan.VkAllocationCallbacks p2, java.nio.LongBuffer p3) { return 0; }
    public static void vkDestroyBufferView(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static void vkDestroyCommandPool(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static void vkDestroyDescriptorSetLayout(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static void vkDestroyDevice(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkAllocationCallbacks p1) {  }
    public static void vkDestroyImageView(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static void vkDestroyInstance(org.lwjgl.vulkan.VkInstance p0, org.lwjgl.vulkan.VkAllocationCallbacks p1) {  }
    public static void vkDestroyPipeline(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static void vkDestroyPipelineLayout(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static void vkDestroyQueryPool(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static void vkDestroySampler(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static void vkDestroySemaphore(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static void vkDestroyShaderModule(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.vulkan.VkAllocationCallbacks p2) {  }
    public static int vkEndCommandBuffer(org.lwjgl.vulkan.VkCommandBuffer p0) { return 0; }
    public static int vkEnumerateDeviceExtensionProperties(org.lwjgl.vulkan.VkPhysicalDevice p0, java.lang.CharSequence p1, java.nio.IntBuffer p2, org.lwjgl.vulkan.VkExtensionProperties.Buffer p3) { return 0; }
    public static int vkEnumerateInstanceExtensionProperties(java.lang.CharSequence p0, java.nio.IntBuffer p1, org.lwjgl.vulkan.VkExtensionProperties.Buffer p2) { return 0; }
    public static int vkEnumerateInstanceLayerProperties(java.nio.IntBuffer p0, org.lwjgl.vulkan.VkLayerProperties.Buffer p1) { return 0; }
    public static int vkEnumeratePhysicalDevices(org.lwjgl.vulkan.VkInstance p0, java.nio.IntBuffer p1, org.lwjgl.PointerBuffer p2) { return 0; }
    public static void vkFreeCommandBuffers(org.lwjgl.vulkan.VkDevice p0, long p1, org.lwjgl.PointerBuffer p2) {  }
    public static void vkGetDeviceQueue(org.lwjgl.vulkan.VkDevice p0, int p1, int p2, org.lwjgl.PointerBuffer p3) {  }
    public static void vkGetPhysicalDeviceFeatures2(org.lwjgl.vulkan.VkPhysicalDevice p0, org.lwjgl.vulkan.VkPhysicalDeviceFeatures2 p1) {  }
    public static void vkGetPhysicalDeviceMemoryProperties(org.lwjgl.vulkan.VkPhysicalDevice p0, org.lwjgl.vulkan.VkPhysicalDeviceMemoryProperties p1) {  }
    public static void vkGetPhysicalDeviceProperties(org.lwjgl.vulkan.VkPhysicalDevice p0, org.lwjgl.vulkan.VkPhysicalDeviceProperties p1) {  }
    public static void vkGetPhysicalDeviceProperties2(org.lwjgl.vulkan.VkPhysicalDevice p0, org.lwjgl.vulkan.VkPhysicalDeviceProperties2 p1) {  }
    public static void vkGetPhysicalDeviceQueueFamilyProperties(org.lwjgl.vulkan.VkPhysicalDevice p0, java.nio.IntBuffer p1, org.lwjgl.vulkan.VkQueueFamilyProperties.Buffer p2) {  }
    public static int vkGetQueryPoolResults(org.lwjgl.vulkan.VkDevice p0, long p1, int p2, int p3, java.nio.LongBuffer p4, long p5, int p6) { return 0; }
    public static int vkQueueWaitIdle(org.lwjgl.vulkan.VkQueue p0) { return 0; }
    public static int vkResetCommandPool(org.lwjgl.vulkan.VkDevice p0, long p1, int p2) { return 0; }
    public static void vkResetQueryPool(org.lwjgl.vulkan.VkDevice p0, long p1, int p2, int p3) {  }
    public static int vkWaitSemaphores(org.lwjgl.vulkan.VkDevice p0, org.lwjgl.vulkan.VkSemaphoreWaitInfo p1, long p2) { return 0; }
}
