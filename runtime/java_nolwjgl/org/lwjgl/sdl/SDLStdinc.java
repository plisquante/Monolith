package org.lwjgl.sdl;

import org.lwjgl.PointerBuffer;
import org.lwjgl.system.MemoryUtil;

import java.nio.IntBuffer;

public final class SDLStdinc {
    private SDLStdinc() {}

    public static void SDL_free(IntBuffer ptr) {
        if (ptr != null) MemoryUtil.memFree(ptr);
    }

    public static void SDL_free(PointerBuffer ptr) {
        if (ptr != null) MemoryUtil.memFree(ptr);
    }
}
