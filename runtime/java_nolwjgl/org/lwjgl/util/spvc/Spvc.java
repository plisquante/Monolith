package org.lwjgl.util.spvc;

import org.lwjgl.PointerBuffer;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.SharedLibrary;
import org.lwjgl.util.MonoShader;

import java.nio.ByteBuffer;
import java.nio.IntBuffer;
import java.nio.charset.StandardCharsets;

/**
 * SPIRV-Cross through libmonoshader.so (org.lwjgl.util.MonoShader), with the call signatures of LWJGL's binding.
 *
 * One deliberate adaptation: the no-LWJGL runtime draws through OpenGL ES (the host's EGL context, see
 * runtime/gl_shim.zig), but the OpenGL backend of the game asks for desktop GLSL 330. The GLSL version option is
 * therefore raised to the OpenGL ES version below and the GLSL_ES option is forced on, so that the generated source
 * compiles on the ES context.
 */
public final class Spvc {
    private static final SharedLibrary LIB = new SharedLibrary.Default("spirv-cross");

    // spirv_cross_c.h: SPVC_COMPILER_OPTION_GLSL_BIT | n
    private static final int OPTION_GLSL_VERSION = 0x2000000 | 8;
    private static final int OPTION_GLSL_ES = 0x2000000 | 9;
    /**
     * OpenGL ES GLSL version emitted for the OpenGL backend. The game asks for desktop GLSL 330 without the
     * 420pack extension, i.e. without binding qualifiers (it binds blocks and samplers through the API); 300 es is the
     * ES version with the same feature set, except that 3.20 has texture buffers in the core (the cloud shaders read
     * from one; 3.00 would need GL_EXT_texture_buffer, which drivers do not accept in vertex shaders).
     * runtime/gl_shim.zig hands `#version ... es` sources to the driver as they are.
     */
    private static final int ES_GLSL_VERSION = 320;

    private Spvc() {}

    public static SharedLibrary getLibrary() { return LIB; }

    public static int spvc_context_create(PointerBuffer context) {
        return MonoShader.spCtxCreate(MemoryUtil.memAddress(context));
    }

    public static void spvc_context_destroy(long context) {
        MonoShader.spCtxDestroy(context);
        synchronized (LOCK) {
            int w = 0;
            for (int i = 0; i < nOwned; i++) {
                if (ownedCtx[i] == context) {
                    MemoryUtil.nmemFree(ownedPtr[i]);
                } else {
                    ownedPtr[w] = ownedPtr[i];
                    ownedCtx[w] = ownedCtx[i];
                    w++;
                }
            }
            nOwned = w;
            w = 0;
            for (int i = 0; i < nCompilers; i++) {
                if (compilerCtx[i] != context) {
                    compilers[w] = compilers[i];
                    compilerCtx[w] = compilerCtx[i];
                    w++;
                }
            }
            nCompilers = w;
        }
    }

    // Rewritten sources live as long as their context, like the strings SPIRV-Cross itself returns. Plain arrays keep
    // this shim free of collections and lambdas (it is compiled on its own; see monolith/shard.py dev units).
    private static final Object LOCK = new Object();
    private static long[] ownedPtr = new long[8];
    private static long[] ownedCtx = new long[8];
    private static int nOwned;
    private static long[] compilers = new long[8];
    private static long[] compilerCtx = new long[8];
    private static int nCompilers;

    private static long contextOf(long compiler) {
        for (int i = 0; i < nCompilers; i++) if (compilers[i] == compiler) return compilerCtx[i];
        throw new IllegalStateException(new StringBuilder().append("SPIRV-Cross compiler ").append(compiler)
                .append(" has no known context").toString());
    }

    private static long keepString(long compiler, String s) {
        byte[] b = s.getBytes(StandardCharsets.UTF_8);
        long p = MemoryUtil.nmemAlloc(b.length + 1L);
        for (int i = 0; i < b.length; i++) MemoryUtil.memPutByte(p + i, b[i]);
        MemoryUtil.memPutByte(p + b.length, (byte) 0);
        synchronized (LOCK) {
            long ctx = contextOf(compiler);
            if (nOwned == ownedPtr.length) {
                ownedPtr = java.util.Arrays.copyOf(ownedPtr, nOwned * 2);
                ownedCtx = java.util.Arrays.copyOf(ownedCtx, nOwned * 2);
            }
            ownedPtr[nOwned] = p;
            ownedCtx[nOwned] = ctx;
            nOwned++;
        }
        return p;
    }

    private static final java.util.regex.Pattern OUT_ARRAY = java.util.regex.Pattern.compile(
            "layout\\(location = (\\d+)\\) out ((?:(?:lowp|mediump|highp) )?)(\\w+) (\\w+)\\[(\\d+)\\];");

    /**
     * GLSL ES requires constant indices into fragment shader output arrays; desktop GLSL does not, and SPIRV-Cross
     * keeps the game's loops over multiple render targets (e.g. "out[attachmentIndex] = ..."). Each output array of a
     * fragment shader becomes a private array plus one output per location, copied after the original main().
     * Returns the input itself when there is nothing to change.
     */
    static String splitFragmentOutputArrays(String src) {
        if (!src.startsWith("#version") || !src.contains(" es\nprecision mediump float;")) return src; // ES fragment only
        java.util.regex.Matcher m = OUT_ARRAY.matcher(src);
        StringBuilder out = new StringBuilder();
        StringBuilder decls = new StringBuilder();
        StringBuilder copies = new StringBuilder();
        int last = 0;
        while (m.find()) {
            int loc = Integer.parseInt(m.group(1));
            int n = Integer.parseInt(m.group(5));
            String prec = m.group(2), type = m.group(3), name = m.group(4);
            out.append(src.substring(last, m.start())).append(prec).append(type).append(' ').append(name).append('[').append(n).append("];");
            last = m.end();
            for (int k = 0; k < n; k++) {
                decls.append("layout(location = ").append(loc + k).append(") out ").append(prec).append(type).append(' ')
                        .append(name).append("_mono").append(k).append(";\n");
                copies.append("    ").append(name).append("_mono").append(k).append(" = ").append(name).append('[').append(k).append("];\n");
            }
        }
        if (last == 0) return src;
        out.append(src.substring(last));
        String s = out.toString();
        int mi = s.lastIndexOf("void main()");
        if (mi < 0) throw new IllegalStateException("SPIRV-Cross output without main(): cannot split fragment outputs");
        return new StringBuilder().append(s.substring(0, mi)).append(decls.toString()).append("\nvoid _mono_main()")
                .append(s.substring(mi + 11)).append("\nvoid main()\n{\n    _mono_main();\n").append(copies.toString())
                .append("}\n").toString();
    }

    public static int spvc_context_parse_spirv(long context, IntBuffer spirv, long word_count, PointerBuffer parsed_ir) {
        return MonoShader.spParse(context, MemoryUtil.memAddress(spirv), word_count, MemoryUtil.memAddress(parsed_ir));
    }

    public static int spvc_context_create_compiler(long context, int backend, long parsed_ir, int mode, PointerBuffer compiler) {
        int r = MonoShader.spCreateCompiler(context, backend, parsed_ir, mode, MemoryUtil.memAddress(compiler));
        if (r == 0) {
            synchronized (LOCK) {
                if (nCompilers == compilers.length) {
                    compilers = java.util.Arrays.copyOf(compilers, nCompilers * 2);
                    compilerCtx = java.util.Arrays.copyOf(compilerCtx, nCompilers * 2);
                }
                compilers[nCompilers] = compiler.get(0);
                compilerCtx[nCompilers] = context;
                nCompilers++;
            }
        }
        return r;
    }

    public static int spvc_compiler_create_compiler_options(long compiler, PointerBuffer options) {
        return MonoShader.spCreateOptions(compiler, MemoryUtil.memAddress(options));
    }

    public static int spvc_compiler_install_compiler_options(long compiler, long options) {
        return MonoShader.spInstallOptions(compiler, options);
    }

    public static int spvc_compiler_options_set_bool(long options, int option, boolean value) {
        if (option == OPTION_GLSL_ES) value = true;
        return MonoShader.spOptBool(options, option, value ? 1 : 0);
    }

    public static int spvc_compiler_options_set_uint(long options, int option, int value) {
        if (option == OPTION_GLSL_VERSION) {
            // the runtime draws through OpenGL ES: switch the generator to ES together with the version
            int r = MonoShader.spOptBool(options, OPTION_GLSL_ES, 1);
            if (r != 0) return r;
            value = ES_GLSL_VERSION;
        }
        return MonoShader.spOptUint(options, option, value);
    }

    private static final String DUMP_DIR = System.getenv("MONO_DUMP_GLSL");
    private static final java.util.concurrent.atomic.AtomicInteger DUMP_COUNTER = new java.util.concurrent.atomic.AtomicInteger();

    public static int spvc_compiler_compile(long compiler, PointerBuffer source) {
        int result = MonoShader.spCompile(compiler, MemoryUtil.memAddress(source));
        if (result == 0) {
            String glsl = MemoryUtil.memUTF8(source.get(0));
            String fixed = splitFragmentOutputArrays(glsl);
            if (fixed != glsl) source.put(0, keepString(compiler, fixed));
        }
        if (DUMP_DIR != null && result == 0) { // debugging aid: MONO_DUMP_GLSL=<dir> writes every translated shader
            try {
                java.nio.file.Files.write(java.nio.file.Path.of(DUMP_DIR, "shader_" + DUMP_COUNTER.incrementAndGet() + ".glsl"),
                        MemoryUtil.memUTF8(source.get(0)).getBytes(StandardCharsets.UTF_8));
            } catch (java.io.IOException e) {
                throw new IllegalStateException("MONO_DUMP_GLSL: " + e.getMessage(), e);
            }
        }
        return result;
    }

    public static int spvc_compiler_create_shader_resources(long compiler, PointerBuffer resources) {
        return MonoShader.spShaderResources(compiler, MemoryUtil.memAddress(resources));
    }

    public static boolean spvc_compiler_get_binary_offset_for_decoration(long compiler, int id, int decoration, IntBuffer word_offset) {
        return MonoShader.spBinaryOffset(compiler, id, decoration, MemoryUtil.memAddress(word_offset)) != 0;
    }

    public static int spvc_compiler_get_declared_struct_size(long compiler, long type, PointerBuffer size) {
        return MonoShader.spStructSize(compiler, type, MemoryUtil.memAddress(size));
    }

    public static int spvc_compiler_get_decoration(long compiler, int id, int decoration) {
        return MonoShader.spDecoration(compiler, id, decoration);
    }

    public static String spvc_compiler_get_name(long compiler, int id) {
        long p = MonoShader.spName(compiler, id);
        return p == 0L ? null : MemoryUtil.memUTF8(p);
    }

    public static long spvc_compiler_get_type_handle(long compiler, int id) {
        return MonoShader.spTypeHandle(compiler, id);
    }

    public static int spvc_compiler_set_entry_point(long compiler, CharSequence name, int model) {
        ByteBuffer n = cString(name);
        try {
            return MonoShader.spSetEntry(compiler, MemoryUtil.memAddress(n), model);
        } finally {
            MemoryUtil.memFree(n);
        }
    }

    public static void spvc_compiler_set_name(long compiler, int id, CharSequence argument) {
        ByteBuffer n = cString(argument);
        try {
            MonoShader.spSetName(compiler, id, MemoryUtil.memAddress(n));
        } finally {
            MemoryUtil.memFree(n);
        }
    }

    public static int spvc_resources_get_resource_list_for_type(long resources, int type, PointerBuffer resource_list, PointerBuffer resource_size) {
        return MonoShader.spResourceList(resources, type, MemoryUtil.memAddress(resource_list), MemoryUtil.memAddress(resource_size));
    }

    public static int spvc_type_get_array_dimension(long type, int dimension) { return MonoShader.spTypeArrayDim(type, dimension); }

    public static int spvc_type_get_basetype(long type) { return MonoShader.spTypeBase(type); }

    public static int spvc_type_get_image_dimension(long type) { return MonoShader.spTypeImageDim(type); }

    public static int spvc_type_get_num_array_dimensions(long type) { return MonoShader.spTypeNumArrayDims(type); }

    public static int spvc_type_get_vector_size(long type) { return MonoShader.spTypeVec(type); }

    /** Message of the last error of a context (null when there is none). */
    public static String spvc_context_get_last_error_string(long context) {
        long p = MonoShader.spLastError(context);
        return p == 0L ? null : MemoryUtil.memUTF8(p);
    }

    private static ByteBuffer cString(CharSequence s) {
        byte[] bytes = s.toString().getBytes(StandardCharsets.UTF_8);
        ByteBuffer b = MemoryUtil.memAlloc(bytes.length + 1);
        b.put(bytes);
        b.put((byte) 0);
        b.flip();
        return b;
    }
}
