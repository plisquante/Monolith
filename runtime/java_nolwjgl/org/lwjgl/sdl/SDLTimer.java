package org.lwjgl.sdl;

public final class SDLTimer {
    private static final long START_NS = System.nanoTime();

    private SDLTimer() {}

    public static long SDL_GetTicksNS() {
        return Math.max(0L, System.nanoTime() - START_NS);
    }
}
