package org.lwjgl.util.shaderc;

@FunctionalInterface
public interface ShadercIncludeResultReleaseI {
    void invoke(long user_data, long include_result);
}
