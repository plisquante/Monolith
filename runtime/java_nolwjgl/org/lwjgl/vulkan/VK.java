package org.lwjgl.vulkan;

public class VK {
    /**
     * The no-LWJGL configuration has no LWJGL Vulkan binding (the RHI brings its own backends). Like LWJGL does
     * when no Vulkan loader exists, fail here: Minecraft's NativeLibrariesBootstrap.tryLoadingVulkan catches it
     * and the game falls back to the OpenGL backend instead of crashing later on a null function provider.
     */
    public static void create() {
        throw new IllegalStateException("Vulkan through LWJGL is not available without LWJGL (RHI mode)");
    }

    public static org.lwjgl.system.FunctionProvider getFunctionProvider() {
        throw new IllegalStateException("Vulkan through LWJGL is not available without LWJGL (RHI mode)");
    }
}
