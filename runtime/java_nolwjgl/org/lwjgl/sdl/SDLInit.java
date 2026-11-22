package org.lwjgl.sdl;

import mono.rhi.NativeRhi;

public final class SDLInit {
    public static final int SDL_INIT_AUDIO = 0x00000010;
    public static final int SDL_INIT_VIDEO = 0x00000020;
    public static final int SDL_INIT_JOYSTICK = 0x00000200;
    public static final int SDL_INIT_HAPTIC = 0x00001000;
    public static final int SDL_INIT_GAMEPAD = 0x00002000;
    public static final int SDL_INIT_EVENTS = 0x00004000;
    public static final int SDL_INIT_SENSOR = 0x00008000;
    public static final int SDL_INIT_CAMERA = 0x00010000;

    private static boolean initialized = false;

    private SDLInit() {}

    public static boolean SDL_Init(int flags) {
        if (!initialized) {
            // -Dmono.graphics.backend=..., else the MONO_RHI_BACKEND environment variable (the Android host sets it)
            String mode = System.getProperty("mono.graphics.backend");
            if (mode == null) mode = System.getenv("MONO_RHI_BACKEND");
            if (mode == null) mode = "auto";
            int b = NativeRhi.BACKEND_AUTO;
            if ("vulkan".equalsIgnoreCase(mode)) b = NativeRhi.BACKEND_VULKAN;
            else if ("opengl".equalsIgnoreCase(mode)) b = NativeRhi.BACKEND_OPENGL33;
            else if ("headless".equalsIgnoreCase(mode)) b = NativeRhi.BACKEND_HEADLESS;
            initialized = NativeRhi.init(b, NativeRhi.POWER_BALANCED) >= 0;
        }
        return initialized;
    }

    public static void SDL_Quit() {
        SDLVideo.resetAll();
        initialized = false;
    }

    public static boolean SDL_SetAppMetadataProperty(CharSequence name, CharSequence value) {
        return true;
    }
}
