package org.lwjgl.sdl;

import java.nio.IntBuffer;

public final class SDLMessageBox {
    private SDLMessageBox() {}

    public static boolean SDL_ShowMessageBox(SDL_MessageBoxData messageboxdata, IntBuffer buttonid) {
        if (buttonid != null && buttonid.remaining() > 0) {
            buttonid.put(buttonid.position(), 0);
        }
        return true;
    }

    public static boolean SDL_ShowSimpleMessageBox(int flags, CharSequence title, CharSequence message, long window) {
        System.err.println("[SDL_MessageBox] " + title + ": " + message);
        return true;
    }
}
