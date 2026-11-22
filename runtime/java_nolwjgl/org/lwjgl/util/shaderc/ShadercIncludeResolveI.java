package org.lwjgl.util.shaderc;

@FunctionalInterface
public interface ShadercIncludeResolveI {
    long invoke(long user_data, long requested_source, int type, long requesting_source, long include_depth);
}
