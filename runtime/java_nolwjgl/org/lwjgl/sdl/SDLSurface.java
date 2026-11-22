package org.lwjgl.sdl;

import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

public final class SDLSurface {
    private SDLSurface() {}

    public static boolean SDL_AddSurfaceAlternateImage(SDL_Surface surface, SDL_Surface image) {
        return surface != null && image != null;
    }

    public static SDL_Surface SDL_CreateSurfaceFrom(int width, int height, int format, ByteBuffer pixels, int pitch) {
        return new SDL_Surface(pixels == null ? 0L : MemoryUtil.memAddress(pixels), pixels, width, height);
    }

    public static void SDL_DestroySurface(SDL_Surface surface) {}
}
