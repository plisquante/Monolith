package org.lwjgl.util.shaderc;

import org.lwjgl.system.Callback;

public abstract class ShadercIncludeResolve extends Callback implements ShadercIncludeResolveI {
    protected ShadercIncludeResolve() { super(); }

    public static ShadercIncludeResolve create(ShadercIncludeResolveI instance) {
        if (instance instanceof ShadercIncludeResolve) return (ShadercIncludeResolve) instance;
        return new ShadercIncludeResolve() {
            @Override
            public long invoke(long user_data, long requested_source, int type, long requesting_source, long include_depth) {
                return instance.invoke(user_data, requested_source, type, requesting_source, include_depth);
            }
        };
    }
}
