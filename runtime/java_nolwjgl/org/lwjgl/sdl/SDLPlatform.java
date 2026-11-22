package org.lwjgl.sdl;

public final class SDLPlatform {
    private SDLPlatform() {}

    public static String SDL_GetPlatform() {
        String os = System.getProperty("os.name", "Linux");
        String lo = os.toLowerCase();
        if (lo.contains("android") || System.getProperty("java.vm.vendor", "").toLowerCase().contains("android")) {
            return "Android";
        }
        if (lo.contains("win")) return "Windows";
        if (lo.contains("mac")) return "macOS";
        return "Linux";
    }
}
