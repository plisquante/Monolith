package org.lwjgl.sdl;

import org.lwjgl.system.SharedLibrary;

public final class SDL {
    private static final SharedLibrary LIB = new SharedLibrary.Default("SDL3");

    private SDL() {}

    public static SharedLibrary getLibrary() {
        return LIB;
    }
}
