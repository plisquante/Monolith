package org.lwjgl.sdl;

import java.nio.FloatBuffer;

public final class SDLMouse {
    public static final int SDL_BUTTON_LEFT = 1;
    public static final int SDL_BUTTON_MIDDLE = 2;
    public static final int SDL_BUTTON_RIGHT = 3;

    static float cursorX = 640.0f;
    static float cursorY = 360.0f;
    static int mouseMask = 0;

    private SDLMouse() {}

    public static long SDL_CreateSystemCursor(int id) { return 1L + id; }
    public static long SDL_GetDefaultCursor() { return 1L; }

    public static int SDL_GetGlobalMouseState(FloatBuffer x, FloatBuffer y) {
        return SDL_GetMouseState(x, y);
    }

    public static int SDL_GetMouseState(FloatBuffer x, FloatBuffer y) {
        if (x != null && x.remaining() > 0) x.put(x.position(), cursorX);
        if (y != null && y.remaining() > 0) y.put(y.position(), cursorY);
        return mouseMask;
    }

    public static boolean SDL_SetCursor(long cursor) { return true; }

    public static boolean SDL_SetWindowRelativeMouseMode(long window, boolean enabled) {
        SDLVideo.relativeMouse = enabled;
        j2z.rhi.NativeRhi.setMouseGrabbed(enabled); // the touch layer follows: cursor mode while a screen is open
        return true;
    }

    public static void SDL_WarpMouseInWindow(long window, float x, float y) {
        cursorX = x;
        cursorY = y;
    }
}
