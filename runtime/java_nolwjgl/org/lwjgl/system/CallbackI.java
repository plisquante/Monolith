package org.lwjgl.system;

public interface CallbackI extends Pointer {
    @Override
    default long address() {
        return 0L;
    }
}
