package org.lwjgl.util;

/**
 * Natives of libmonoshader.so (android/shader/monoshader.c): one-to-one forwards of the shaderc and SPIRV-Cross C APIs.
 * All arguments are integers or native addresses; org.lwjgl.util.shaderc.Shaderc and org.lwjgl.util.spvc.Spvc do the
 * marshalling. The library is found in the natives directory (MONO_NATIVES, or natives next to the executable).
 */
public final class MonoShader {
    static {
        System.loadLibrary("monoshader");
    }

    private MonoShader() {}

    // ---- shaderc
    public static native long scCompilerInit();
    public static native void scCompilerRelease(long compiler);
    public static native long scOptInit();
    public static native void scOptRelease(long options);
    public static native void scOptMacro(long options, long name, long nameLen, long value, long valueLen);
    public static native void scOptAutoBind(long options, int enable);
    public static native void scOptDebug(long options);
    public static native void scOptLevel(long options, int level);
    public static native void scOptTarget(long options, int target, int version);
    public static native long scCompile(long compiler, long source, long sourceLen, int kind, long fileName, long entry, long options);
    public static native void scResultRelease(long result);
    public static native int scResultStatus(long result);
    public static native long scResultMessage(long result);
    public static native long scResultBytes(long result);
    public static native long scResultLength(long result);

    // ---- SPIRV-Cross
    public static native int spCtxCreate(long out);
    public static native void spCtxDestroy(long context);
    public static native long spLastError(long context);
    public static native int spParse(long context, long spirv, long words, long outIr);
    public static native int spCreateCompiler(long context, int backend, long ir, int mode, long out);
    public static native int spCreateOptions(long compiler, long out);
    public static native int spInstallOptions(long compiler, long options);
    public static native int spOptBool(long options, int option, int value);
    public static native int spOptUint(long options, int option, int value);
    public static native int spCompile(long compiler, long out);
    public static native int spShaderResources(long compiler, long out);
    public static native int spBinaryOffset(long compiler, int id, int decoration, long out);
    public static native int spStructSize(long compiler, long type, long out);
    public static native int spDecoration(long compiler, int id, int decoration);
    public static native long spName(long compiler, int id);
    public static native long spTypeHandle(long compiler, int id);
    public static native int spSetEntry(long compiler, long name, int model);
    public static native void spSetName(long compiler, int id, long name);
    public static native int spResourceList(long resources, int type, long outList, long outCount);
    public static native int spTypeArrayDim(long type, int dimension);
    public static native int spTypeBase(long type);
    public static native int spTypeImageDim(long type);
    public static native int spTypeNumArrayDims(long type);
    public static native int spTypeVec(long type);
}
