package org.lwjgl.vulkan;

public class VkGraphicsPipelineCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkGraphicsPipelineCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkGraphicsPipelineCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer calloc(int p0, org.lwjgl.system.MemoryStack p1) { java.nio.ByteBuffer b = p1.calloc(8, Math.max(1, p0) * 64); return new Buffer(b); }

    public static class Buffer extends org.lwjgl.system.StructBuffer implements org.lwjgl.system.NativeResource {
        public Buffer(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / 64, container.remaining() / 64); }
        public Buffer(long address, int capacity) { super(address, null, -1, 0, capacity, capacity); }
        @Override public int sizeof() { return 64; }
        @Override public VkGraphicsPipelineCreateInfo get(int index) { return new VkGraphicsPipelineCreateInfo(address + (long) index * 64L, container); }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer flags(int p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer layout(long p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pColorBlendState(org.lwjgl.vulkan.VkPipelineColorBlendStateCreateInfo p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pDepthStencilState(org.lwjgl.vulkan.VkPipelineDepthStencilStateCreateInfo p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pDynamicState(org.lwjgl.vulkan.VkPipelineDynamicStateCreateInfo p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pInputAssemblyState(org.lwjgl.vulkan.VkPipelineInputAssemblyStateCreateInfo p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pMultisampleState(org.lwjgl.vulkan.VkPipelineMultisampleStateCreateInfo p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pNext(org.lwjgl.vulkan.VkPipelineRenderingCreateInfoKHR p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pRasterizationState(org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pStages(org.lwjgl.vulkan.VkPipelineShaderStageCreateInfo.Buffer p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pVertexInputState(org.lwjgl.vulkan.VkPipelineVertexInputStateCreateInfo p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer pViewportState(org.lwjgl.vulkan.VkPipelineViewportStateCreateInfo p0) { return this; }
        public org.lwjgl.vulkan.VkGraphicsPipelineCreateInfo.Buffer sType$Default() { return this; }
    }
}
