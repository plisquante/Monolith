package org.lwjgl.opengl;

import org.lwjgl.system.FunctionProvider;

public final class GL {
    private static final GLCapabilities CAPS = new GLCapabilities();
    /**
     * GlBackend.loadLibrary casts the provider to SharedLibrary, asks for its path (handed to SDL_GL_LoadLibrary) and
     * checks that its glGetError address equals SDL_GL_GetProcAddress's: the provider is the "GL library" of the
     * RHI/SDL shim and resolves functions through the same shim.
     */
    private static final class ShimLibrary implements org.lwjgl.system.SharedLibrary {
        @Override public String getName() { return "GL"; }
        @Override public String getPath() { return null; }
        @Override public long address() { return 1L; }
        @Override public long getFunctionAddress(CharSequence functionName) {
            return org.lwjgl.sdl.SDLVideo.SDL_GL_GetProcAddress(functionName);
        }
    }

    private static final FunctionProvider PROVIDER = new ShimLibrary();

    private GL() {}

    public static GLCapabilities createCapabilities() {
        return CAPS;
    }

    public static GLCapabilities getCapabilities() {
        return CAPS;
    }

    public static FunctionProvider getFunctionProvider() {
        return PROVIDER;
    }
}
