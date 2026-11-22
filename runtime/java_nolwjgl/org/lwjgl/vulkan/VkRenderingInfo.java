package org.lwjgl.vulkan;

public class VkRenderingInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkRenderingInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkRenderingInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkRenderingInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkRenderingInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkRenderingInfo layerCount(int p0) { return this; }
    public org.lwjgl.vulkan.VkRenderingInfo pColorAttachments(org.lwjgl.vulkan.VkRenderingAttachmentInfo.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkRenderingInfo pDepthAttachment(org.lwjgl.vulkan.VkRenderingAttachmentInfo p0) { return this; }
    public org.lwjgl.vulkan.VkRenderingInfo renderArea(org.lwjgl.vulkan.VkRect2D p0) { return this; }
    public org.lwjgl.vulkan.VkRenderingInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkRenderingInfo viewMask(int p0) { return this; }
}
