package org.lwjgl.sdl;

import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

public final class SDLPixels {
    private static final ByteBuffer DEFAULT_BUF = MemoryUtil.memCalloc(32);
    private static final SDL_PixelFormatDetails DEFAULT_DETAILS =
            new SDL_PixelFormatDetails(MemoryUtil.memAddress(DEFAULT_BUF), DEFAULT_BUF);

    private SDLPixels() {}

    public static SDL_PixelFormatDetails SDL_GetPixelFormatDetails(int format) {
        return DEFAULT_DETAILS;
    }
}
