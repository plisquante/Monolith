package org.lwjgl.sdl;

public final class SDLError {
    private static String lastError = "";

    private SDLError() {}

    public static String SDL_GetError() {
        return lastError;
    }

    static void setError(String msg) {
        lastError = (msg != null) ? msg : "";
    }
}
