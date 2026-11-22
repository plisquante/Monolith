package org.lwjgl.vulkan;

public class VkPhysicalDeviceVertexAttributeDivisorFeaturesEXT extends org.lwjgl.vulkan.VkPhysicalDeviceVertexAttributeDivisorFeatures {
    public VkPhysicalDeviceVertexAttributeDivisorFeaturesEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPhysicalDeviceVertexAttributeDivisorFeaturesEXT(java.nio.ByteBuffer container) { super(container); }
    public static int PNEXT = 0;
    public static int STYPE = 0;
    public static int VERTEXATTRIBUTEINSTANCERATEDIVISOR = 0;
    public boolean vertexAttributeInstanceRateDivisor() { return false; }
    public static org.lwjgl.vulkan.VkPhysicalDeviceVertexAttributeDivisorFeaturesEXT calloc() { java.nio.ByteBuffer b = org.lwjgl.system.MemoryUtil.memCalloc(64); return new VkPhysicalDeviceVertexAttributeDivisorFeaturesEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public static org.lwjgl.vulkan.VkPhysicalDeviceVertexAttributeDivisorFeaturesEXT calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPhysicalDeviceVertexAttributeDivisorFeaturesEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPhysicalDeviceVertexAttributeDivisorFeaturesEXT sType$Default() { return this; }
    public long pNext() { return 0L; }
    public void free() {  }
}
