package org.lwjgl.util.shaderc;

import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.SharedLibrary;
import org.lwjgl.util.MonoShader;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ConcurrentHashMap;

/**
 * shaderc through libmonoshader.so (org.lwjgl.util.MonoShader). The call signatures are the ones of LWJGL's binding.
 * One difference: #include is resolved here, in Java, before the source reaches shaderc. The resolver and releaser a
 * caller registers with shaderc_compile_options_set_include_callbacks are plain Java objects, and the C library has
 * no way to call back into the runtime.
 */
public final class Shaderc {
    private static final SharedLibrary LIB = new SharedLibrary.Default("shaderc");
    private static final int MAX_INCLUDE_DEPTH = 32;

    private static final class IncludeConfig {
        final ShadercIncludeResolveI resolver;
        final ShadercIncludeResultReleaseI releaser;
        final long userData;

        IncludeConfig(ShadercIncludeResolveI resolver, ShadercIncludeResultReleaseI releaser, long userData) {
            this.resolver = resolver;
            this.releaser = releaser;
            this.userData = userData;
        }
    }

    private static final ConcurrentHashMap<Long, IncludeConfig> INCLUDES = new ConcurrentHashMap<Long, IncludeConfig>();
    private static final ConcurrentHashMap<Long, Boolean> PRESERVE_BINDINGS = new ConcurrentHashMap<Long, Boolean>();

    private Shaderc() {}

    public static SharedLibrary getLibrary() { return LIB; }

    public static long shaderc_compiler_initialize() { return MonoShader.scCompilerInit(); }

    public static void shaderc_compiler_release(long compiler) { MonoShader.scCompilerRelease(compiler); }

    public static long shaderc_compile_options_initialize() { return MonoShader.scOptInit(); }

    public static void shaderc_compile_options_release(long options) {
        INCLUDES.remove(options);
        PRESERVE_BINDINGS.remove(options);
        MonoShader.scOptRelease(options);
    }

    public static void shaderc_compile_options_add_macro_definition(long options, CharSequence name, CharSequence value) {
        byte[] n = name.toString().getBytes(StandardCharsets.UTF_8);
        byte[] v = value == null ? new byte[0] : value.toString().getBytes(StandardCharsets.UTF_8);
        ByteBuffer nb = cString(n);
        ByteBuffer vb = cString(v);
        try {
            MonoShader.scOptMacro(options, MemoryUtil.memAddress(nb), n.length, MemoryUtil.memAddress(vb), v.length);
        } finally {
            MemoryUtil.memFree(nb);
            MemoryUtil.memFree(vb);
        }
    }

    public static void shaderc_compile_options_set_auto_bind_uniforms(long options, boolean auto_bind) {
        MonoShader.scOptAutoBind(options, auto_bind ? 1 : 0);
    }

    public static void shaderc_compile_options_set_generate_debug_info(long options) {
        MonoShader.scOptDebug(options);
    }

    public static void shaderc_compile_options_set_include_callbacks(long options, ShadercIncludeResolveI resolver,
                                                                    ShadercIncludeResultReleaseI result_releaser, long user_data) {
        if (resolver == null) {
            INCLUDES.remove(options);
        } else {
            INCLUDES.put(options, new IncludeConfig(resolver, result_releaser, user_data));
        }
    }

    /** shaderc_optimization_level_zero: the optimizer, the only thing that can drop bindings, does not run. */
    private static final int OPTIMIZATION_ZERO = 0;

    public static void shaderc_compile_options_set_optimization_level(long options, int level) {
        MonoShader.scOptLevel(options, PRESERVE_BINDINGS.containsKey(options) ? OPTIMIZATION_ZERO : level);
    }

    /**
     * The shaderc the Android NDK ships (2022.3) has no shaderc_compile_options_set_preserve_bindings. Bindings are only
     * ever removed by the optimizer, so preserving them is done by never running it for these options.
     */
    public static void shaderc_compile_options_set_preserve_bindings(long options, boolean preserve_bindings) {
        if (preserve_bindings) {
            PRESERVE_BINDINGS.put(options, Boolean.TRUE);
            MonoShader.scOptLevel(options, OPTIMIZATION_ZERO);
        } else {
            PRESERVE_BINDINGS.remove(options);
        }
    }

    public static void shaderc_compile_options_set_target_env(long options, int target, int version) {
        MonoShader.scOptTarget(options, target, version);
    }

    public static long shaderc_compile_into_spv(long compiler, ByteBuffer source_text, int shader_kind,
                                                ByteBuffer input_file_name, ByteBuffer entry_point_name, long additional_options) {
        byte[] raw = new byte[source_text.remaining()];
        source_text.duplicate().get(raw);
        IncludeConfig inc = INCLUDES.get(additional_options);
        if (inc != null) {
            String text = new String(raw, StandardCharsets.UTF_8);
            raw = expandIncludes(text, inc, MemoryUtil.memASCII(MemoryUtil.memAddress(input_file_name)), 0).getBytes(StandardCharsets.UTF_8);
        }
        ByteBuffer src = cString(raw);
        try {
            return MonoShader.scCompile(compiler, MemoryUtil.memAddress(src), raw.length, shader_kind,
                    MemoryUtil.memAddress(input_file_name), MemoryUtil.memAddress(entry_point_name), additional_options);
        } finally {
            MemoryUtil.memFree(src);
        }
    }

    public static void shaderc_result_release(long result) { MonoShader.scResultRelease(result); }

    public static int shaderc_result_get_compilation_status(long result) { return MonoShader.scResultStatus(result); }

    public static String shaderc_result_get_error_message(long result) {
        long p = MonoShader.scResultMessage(result);
        return p == 0L ? "" : MemoryUtil.memUTF8(p);
    }

    public static ByteBuffer shaderc_result_get_bytes(long result) {
        long p = MonoShader.scResultBytes(result);
        long len = MonoShader.scResultLength(result);
        return p == 0L ? null : MemoryUtil.memByteBuffer(p, (int) len);
    }

    /** NUL-terminated copy in native memory; the caller frees it. */
    private static ByteBuffer cString(byte[] bytes) {
        ByteBuffer b = MemoryUtil.memAlloc(bytes.length + 1);
        b.put(bytes);
        b.put((byte) 0);
        b.flip();
        return b;
    }

    /** Textual #include expansion with the registered resolver (type 0: "relative", 1: &lt;standard&gt;). */
    private static String expandIncludes(String text, IncludeConfig inc, String requesting, int depth) {
        if (depth > MAX_INCLUDE_DEPTH) {
            throw new IllegalStateException("#include nested deeper than " + MAX_INCLUDE_DEPTH + " in " + requesting);
        }
        StringBuilder out = new StringBuilder(text.length() + 256);
        int pos = 0;
        while (pos < text.length()) {
            int eol = text.indexOf('\n', pos);
            int end = eol < 0 ? text.length() : eol + 1;
            String line = text.substring(pos, end);
            String t = line.trim();
            if (t.startsWith("#include")) {
                String rest = t.substring("#include".length()).trim();
                if (rest.length() >= 2 && (rest.charAt(0) == '"' || rest.charAt(0) == '<')) {
                    char close = rest.charAt(0) == '"' ? '"' : '>';
                    int e = rest.indexOf(close, 1);
                    if (e > 0) {
                        String name = rest.substring(1, e);
                        out.append(resolve(name, rest.charAt(0) == '"' ? 0 : 1, requesting, inc, depth));
                        if (eol >= 0) out.append('\n');
                        pos = end;
                        continue;
                    }
                }
            }
            out.append(line);
            pos = end;
        }
        return out.toString();
    }

    private static String resolve(String name, int type, String requesting, IncludeConfig inc, int depth) {
        ByteBuffer req = cString(name.getBytes(StandardCharsets.UTF_8));
        ByteBuffer from = cString(requesting.getBytes(StandardCharsets.UTF_8));
        try {
            long result = inc.resolver.invoke(inc.userData, MemoryUtil.memAddress(req), type, MemoryUtil.memAddress(from), depth + 1);
            if (result == 0L) {
                throw new IllegalStateException("#include \"" + name + "\" could not be resolved (from " + requesting + ")");
            }
            try {
                long contentPtr = MemoryUtil.memGetAddress(result + ShadercIncludeResult.CONTENT);
                long contentLen = MemoryUtil.memGetAddress(result + ShadercIncludeResult.CONTENT + 8);
                long namePtr = MemoryUtil.memGetAddress(result + ShadercIncludeResult.SOURCE_NAME);
                long nameLen = MemoryUtil.memGetAddress(result + ShadercIncludeResult.SOURCE_NAME + 8);
                String content = new String(bytes(contentPtr, contentLen), StandardCharsets.UTF_8);
                String resolvedName = namePtr == 0L ? name : new String(bytes(namePtr, nameLen), StandardCharsets.UTF_8);
                return expandIncludes(content, inc, resolvedName, depth + 1);
            } finally {
                if (inc.releaser != null) inc.releaser.invoke(inc.userData, result);
            }
        } finally {
            MemoryUtil.memFree(req);
            MemoryUtil.memFree(from);
        }
    }

    private static byte[] bytes(long address, long length) {
        byte[] b = new byte[(int) length];
        MemoryUtil.memByteBuffer(address, (int) length).get(b);
        return b;
    }
}
