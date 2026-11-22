package org.lwjgl.sdl;

import org.lwjgl.system.Callback;

public abstract class SDL_LogOutputFunction extends Callback implements SDL_LogOutputFunctionI {
    protected SDL_LogOutputFunction() { super(); }

    public static SDL_LogOutputFunction create(SDL_LogOutputFunctionI instance) {
        if (instance instanceof SDL_LogOutputFunction) {
            return (SDL_LogOutputFunction) instance;
        }
        return new SDL_LogOutputFunction() {
            @Override
            public void invoke(long userdata, int category, int priority, long message) {
                instance.invoke(userdata, category, priority, message);
            }
        };
    }
}
