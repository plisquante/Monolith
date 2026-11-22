package org.lwjgl.openal;

import org.lwjgl.system.FunctionProviderLocal;

public final class ALC {
    private static final ALCCapabilities CAPS = new ALCCapabilities();
    private static final FunctionProviderLocal PROVIDER = name -> 0L;

    private ALC() {}

    public static ALCCapabilities createCapabilities(long device) { return CAPS; }
    public static FunctionProviderLocal getFunctionProvider() { return PROVIDER; }
    public static void destroy() {}
}
