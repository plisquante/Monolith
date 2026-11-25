/*
 * libmonoshader.so: LWJGL'siz motor için shaderc (GLSL -> SPIR-V) ve SPIRV-Cross (SPIR-V -> GLSL).
 *
 * Minecraft 26.x shader'larını çalışma anında derler: renderpearl, GLSL'i shaderc ile SPIR-V'ye çevirir; OpenGL
 * arka ucu da bunu SPIRV-Cross ile tekrar GLSL'e çevirir. LWJGL bu iki kütüphaneyi kendi içinde getirir; LWJGL'siz
 * çalışma zamanı getirmez. Bu yüzden org.lwjgl.util.shaderc.Shaderc ve org.lwjgl.util.spvc.Spvc (runtime/java_nolwjgl)
 * bu kütüphaneyi org.lwjgl.util.MonoShader üzerinden çağırır. Her işlev, C API'sinin düz tamsayılar ve adreslerle
 * birebir iletilmiş hâlidir; bütün veri dönüştürme işi Java tarafındadır.
 */
#include <jni.h>
#include <stdint.h>
#include <string.h>

#include <shaderc/shaderc.h>
#include <spirv_cross_c.h>

#define FN(ret, name, ...) JNIEXPORT ret JNICALL Java_org_lwjgl_util_MonoShader_##name(JNIEnv *env, jclass cls, ##__VA_ARGS__)
#define UNUSED (void)env; (void)cls
#define P(x) ((void *)(uintptr_t)(x))
#define J(x) ((jlong)(uintptr_t)(x))

/* ---- shaderc (GLSL'i SPIR-V'ye çevirir) ---- */
FN(jlong, scCompilerInit) { UNUSED; return J(shaderc_compiler_initialize()); }
FN(void, scCompilerRelease, jlong c) { UNUSED; shaderc_compiler_release(P(c)); }
FN(jlong, scOptInit) { UNUSED; return J(shaderc_compile_options_initialize()); }
FN(void, scOptRelease, jlong o) { UNUSED; shaderc_compile_options_release(P(o)); }
FN(void, scOptMacro, jlong o, jlong name, jlong nameLen, jlong value, jlong valueLen) {
    UNUSED;
    shaderc_compile_options_add_macro_definition(P(o), P(name), (size_t)nameLen, P(value), (size_t)valueLen);
}
FN(void, scOptAutoBind, jlong o, jint v) { UNUSED; shaderc_compile_options_set_auto_bind_uniforms(P(o), v != 0); }
FN(void, scOptDebug, jlong o) { UNUSED; shaderc_compile_options_set_generate_debug_info(P(o)); }
FN(void, scOptLevel, jlong o, jint level) { UNUSED; shaderc_compile_options_set_optimization_level(P(o), (shaderc_optimization_level)level); }
FN(void, scOptTarget, jlong o, jint target, jint version) {
    UNUSED;
    shaderc_compile_options_set_target_env(P(o), (shaderc_target_env)target, (uint32_t)version);
}
FN(jlong, scCompile, jlong c, jlong src, jlong srcLen, jint kind, jlong file, jlong entry, jlong opt) {
    UNUSED;
    return J(shaderc_compile_into_spv(P(c), P(src), (size_t)srcLen, (shaderc_shader_kind)kind, P(file), P(entry), P(opt)));
}
FN(void, scResultRelease, jlong r) { UNUSED; shaderc_result_release(P(r)); }
FN(jint, scResultStatus, jlong r) { UNUSED; return (jint)shaderc_result_get_compilation_status(P(r)); }
FN(jlong, scResultMessage, jlong r) { UNUSED; return J(shaderc_result_get_error_message(P(r))); }
FN(jlong, scResultBytes, jlong r) { UNUSED; return J(shaderc_result_get_bytes(P(r))); }
FN(jlong, scResultLength, jlong r) { UNUSED; return (jlong)shaderc_result_get_length(P(r)); }

/* ---- SPIRV-Cross (SPIR-V'yi tekrar GLSL'e çevirir) ---- */
FN(jint, spCtxCreate, jlong out) { UNUSED; return (jint)spvc_context_create((spvc_context *)P(out)); }
FN(void, spCtxDestroy, jlong ctx) { UNUSED; spvc_context_destroy(P(ctx)); }
FN(jlong, spLastError, jlong ctx) { UNUSED; return J(spvc_context_get_last_error_string(P(ctx))); }
FN(jint, spParse, jlong ctx, jlong spirv, jlong words, jlong outIr) {
    UNUSED;
    return (jint)spvc_context_parse_spirv(P(ctx), P(spirv), (size_t)words, (spvc_parsed_ir *)P(outIr));
}
FN(jint, spCreateCompiler, jlong ctx, jint backend, jlong ir, jint mode, jlong out) {
    UNUSED;
    return (jint)spvc_context_create_compiler(P(ctx), (spvc_backend)backend, P(ir), (spvc_capture_mode)mode, (spvc_compiler *)P(out));
}
FN(jint, spCreateOptions, jlong comp, jlong out) { UNUSED; return (jint)spvc_compiler_create_compiler_options(P(comp), (spvc_compiler_options *)P(out)); }
FN(jint, spInstallOptions, jlong comp, jlong opt) { UNUSED; return (jint)spvc_compiler_install_compiler_options(P(comp), P(opt)); }
FN(jint, spOptBool, jlong opt, jint option, jint v) {
    UNUSED;
    return (jint)spvc_compiler_options_set_bool(P(opt), (spvc_compiler_option)option, v != 0);
}
FN(jint, spOptUint, jlong opt, jint option, jint v) {
    UNUSED;
    return (jint)spvc_compiler_options_set_uint(P(opt), (spvc_compiler_option)option, (unsigned)v);
}
FN(jint, spCompile, jlong comp, jlong out) { UNUSED; return (jint)spvc_compiler_compile(P(comp), (const char **)P(out)); }
FN(jint, spShaderResources, jlong comp, jlong out) { UNUSED; return (jint)spvc_compiler_create_shader_resources(P(comp), (spvc_resources *)P(out)); }
FN(jint, spBinaryOffset, jlong comp, jint id, jint deco, jlong out) {
    UNUSED;
    return spvc_compiler_get_binary_offset_for_decoration(P(comp), (spvc_variable_id)id, (SpvDecoration)deco, (unsigned *)P(out)) ? 1 : 0;
}
FN(jint, spStructSize, jlong comp, jlong type, jlong out) {
    UNUSED;
    return (jint)spvc_compiler_get_declared_struct_size(P(comp), P(type), (size_t *)P(out));
}
FN(jint, spDecoration, jlong comp, jint id, jint deco) { UNUSED; return (jint)spvc_compiler_get_decoration(P(comp), (SpvId)id, (SpvDecoration)deco); }
FN(jlong, spName, jlong comp, jint id) { UNUSED; return J(spvc_compiler_get_name(P(comp), (SpvId)id)); }
FN(jlong, spTypeHandle, jlong comp, jint id) { UNUSED; return J(spvc_compiler_get_type_handle(P(comp), (spvc_type_id)id)); }
FN(jint, spSetEntry, jlong comp, jlong name, jint model) {
    UNUSED;
    return (jint)spvc_compiler_set_entry_point(P(comp), P(name), (SpvExecutionModel)model);
}
FN(void, spSetName, jlong comp, jint id, jlong name) { UNUSED; spvc_compiler_set_name(P(comp), (SpvId)id, P(name)); }
FN(jint, spResourceList, jlong res, jint type, jlong outList, jlong outCount) {
    UNUSED;
    return (jint)spvc_resources_get_resource_list_for_type(P(res), (spvc_resource_type)type,
                                                           (const spvc_reflected_resource **)P(outList), (size_t *)P(outCount));
}
FN(jint, spTypeArrayDim, jlong t, jint d) { UNUSED; return (jint)spvc_type_get_array_dimension(P(t), (unsigned)d); }
FN(jint, spTypeBase, jlong t) { UNUSED; return (jint)spvc_type_get_basetype(P(t)); }
FN(jint, spTypeImageDim, jlong t) { UNUSED; return (jint)spvc_type_get_image_dimension(P(t)); }
FN(jint, spTypeNumArrayDims, jlong t) { UNUSED; return (jint)spvc_type_get_num_array_dimensions(P(t)); }
FN(jint, spTypeVec, jlong t) { UNUSED; return (jint)spvc_type_get_vector_size(P(t)); }
