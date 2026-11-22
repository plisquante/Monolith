package org.lwjgl.sdl;

public final class SDLLog {
    private static SDL_LogOutputFunctionI logCallback;

    private SDLLog() {}

    public static void SDL_SetLogOutputFunction(SDL_LogOutputFunctionI callback, long userdata) {
        logCallback = callback;
    }

    public static void SDL_SetLogPriorities(int priority) {}
}
