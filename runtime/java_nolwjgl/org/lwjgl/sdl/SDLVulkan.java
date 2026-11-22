package org.lwjgl.sdl;

import org.lwjgl.PointerBuffer;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.vulkan.VkAllocationCallbacks;
import org.lwjgl.vulkan.VkInstance;
import org.lwjgl.vulkan.VkPhysicalDevice;

import java.nio.LongBuffer;

public final class SDLVulkan {
    private SDLVulkan() {}

    public static boolean SDL_Vulkan_CreateSurface(
            long window, VkInstance instance, VkAllocationCallbacks allocator, LongBuffer surface) {
        if (surface != null && surface.remaining() > 0) {
            surface.put(surface.position(), 0L);
        }
        return false;
    }

    public static PointerBuffer SDL_Vulkan_GetInstanceExtensions() {
        return MemoryUtil.memAllocPointer(0);
    }

    public static boolean SDL_Vulkan_GetPresentationSupport(
            VkInstance instance, VkPhysicalDevice physicalDevice, int queueFamilyIndex) {
        return false;
    }

    public static long SDL_Vulkan_GetVkGetInstanceProcAddr() { return 0L; }
    public static boolean SDL_Vulkan_LoadLibrary(CharSequence path) { return false; }
    public static void SDL_Vulkan_UnloadLibrary() {}
}
