package org.lwjgl.util.shaderc;

import org.lwjgl.system.Callback;

public abstract class ShadercIncludeResultRelease extends Callback implements ShadercIncludeResultReleaseI {
    protected ShadercIncludeResultRelease() { super(); }

    public static ShadercIncludeResultRelease create(ShadercIncludeResultReleaseI instance) {
        if (instance instanceof ShadercIncludeResultRelease) return (ShadercIncludeResultRelease) instance;
        return new ShadercIncludeResultRelease() {
            @Override
            public void invoke(long user_data, long include_result) {
                instance.invoke(user_data, include_result);
            }
        };
    }
}
