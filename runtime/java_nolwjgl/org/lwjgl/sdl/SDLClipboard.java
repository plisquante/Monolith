package org.lwjgl.sdl;

public final class SDLClipboard {
    private static String clipboard = "";

    private SDLClipboard() {}

    public static String SDL_GetClipboardText() {
        return clipboard;
    }

    public static boolean SDL_SetClipboardText(CharSequence text) {
        clipboard = (text != null) ? text.toString() : "";
        return true;
    }
}
