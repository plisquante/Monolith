/* C variadic trampolines for JNI Call*Method{,V} and NewObject{,V}.
 * Zig 0.15.1 disables @cVaStart / std.builtin.VaList on stage2_llvm aarch64-linux;
 * delegating va_start / va_copy / va_arg to C handles every ABI cleanly via <stdarg.h>.
 */
#include <stdarg.h>
#include <stdint.h>

typedef union {
    uint8_t z;
    int8_t b;
    uint16_t c;
    int16_t s;
    int32_t i;
    int64_t j;
    float f;
    double d;
    void *l;
} mono_jvalue;

struct mono_va_wrap {
    va_list ap;
};

typedef mono_jvalue (*mono_docall_fn)(uint8_t k, void *recv, void *id, void *ap, uint8_t virt);
typedef void *(*mono_newobj_fn)(void *cls, void *id, void *ap);

static mono_docall_fn cb_doCall = 0;
static mono_newobj_fn cb_newObj = 0;

void mono_jni_set_va_cbs(void *dc, void *no) {
    __atomic_store_n(&cb_doCall, (mono_docall_fn)dc, __ATOMIC_RELEASE);
    __atomic_store_n(&cb_newObj, (mono_newobj_fn)no, __ATOMIC_RELEASE);
}

void mono_va_next(void *vp, uint8_t d, mono_jvalue *out) {
    struct mono_va_wrap *w = (struct mono_va_wrap *)vp;
    out->j = 0;
    switch (d) {
        case 'Z': case 'B': case 'C': case 'S': case 'I':
            out->i = va_arg(w->ap, int);
            break;
        case 'J':
            out->j = va_arg(w->ap, int64_t);
            break;
        case 'F': {
            double x = va_arg(w->ap, double);
            out->f = (float)x;
            break;
        }
        case 'D':
            out->d = va_arg(w->ap, double);
            break;
        default:
            out->l = va_arg(w->ap, void *);
            break;
    }
}

#define MONO_CALL_FAMILY(K, CTYPE, FIELD) \
CTYPE mono_jni_virt_##K##_plain(void *e, void *o, void *id, ...) { \
    (void)e; \
    struct mono_va_wrap w; \
    va_start(w.ap, id); \
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE); \
    mono_jvalue r = fn((uint8_t)#K[0], o, id, &w, 1); \
    va_end(w.ap); \
    return r.FIELD; \
} \
CTYPE mono_jni_virt_##K##_v(void *e, void *o, void *id, va_list args) { \
    (void)e; \
    struct mono_va_wrap w; \
    va_copy(w.ap, args); \
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE); \
    mono_jvalue r = fn((uint8_t)#K[0], o, id, &w, 1); \
    va_end(w.ap); \
    return r.FIELD; \
} \
CTYPE mono_jni_nv_##K##_plain(void *e, void *o, void *c, void *id, ...) { \
    (void)e; (void)c; \
    struct mono_va_wrap w; \
    va_start(w.ap, id); \
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE); \
    mono_jvalue r = fn((uint8_t)#K[0], o, id, &w, 0); \
    va_end(w.ap); \
    return r.FIELD; \
} \
CTYPE mono_jni_nv_##K##_v(void *e, void *o, void *c, void *id, va_list args) { \
    (void)e; (void)c; \
    struct mono_va_wrap w; \
    va_copy(w.ap, args); \
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE); \
    mono_jvalue r = fn((uint8_t)#K[0], o, id, &w, 0); \
    va_end(w.ap); \
    return r.FIELD; \
} \
CTYPE mono_jni_st_##K##_plain(void *e, void *c, void *id, ...) { \
    (void)e; (void)c; \
    struct mono_va_wrap w; \
    va_start(w.ap, id); \
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE); \
    mono_jvalue r = fn((uint8_t)#K[0], 0, id, &w, 0); \
    va_end(w.ap); \
    return r.FIELD; \
} \
CTYPE mono_jni_st_##K##_v(void *e, void *c, void *id, va_list args) { \
    (void)e; (void)c; \
    struct mono_va_wrap w; \
    va_copy(w.ap, args); \
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE); \
    mono_jvalue r = fn((uint8_t)#K[0], 0, id, &w, 0); \
    va_end(w.ap); \
    return r.FIELD; \
}

MONO_CALL_FAMILY(L, void *, l)
MONO_CALL_FAMILY(Z, uint8_t, z)
MONO_CALL_FAMILY(B, int8_t, b)
MONO_CALL_FAMILY(C, uint16_t, c)
MONO_CALL_FAMILY(S, int16_t, s)
MONO_CALL_FAMILY(I, int32_t, i)
MONO_CALL_FAMILY(J, int64_t, j)
MONO_CALL_FAMILY(F, float, f)
MONO_CALL_FAMILY(D, double, d)

void mono_jni_virt_V_plain(void *e, void *o, void *id, ...) {
    (void)e;
    struct mono_va_wrap w;
    va_start(w.ap, id);
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE);
    (void)fn('V', o, id, &w, 1);
    va_end(w.ap);
}
void mono_jni_virt_V_v(void *e, void *o, void *id, va_list args) {
    (void)e;
    struct mono_va_wrap w;
    va_copy(w.ap, args);
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE);
    (void)fn('V', o, id, &w, 1);
    va_end(w.ap);
}
void mono_jni_nv_V_plain(void *e, void *o, void *c, void *id, ...) {
    (void)e; (void)c;
    struct mono_va_wrap w;
    va_start(w.ap, id);
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE);
    (void)fn('V', o, id, &w, 0);
    va_end(w.ap);
}
void mono_jni_nv_V_v(void *e, void *o, void *c, void *id, va_list args) {
    (void)e; (void)c;
    struct mono_va_wrap w;
    va_copy(w.ap, args);
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE);
    (void)fn('V', o, id, &w, 0);
    va_end(w.ap);
}
void mono_jni_st_V_plain(void *e, void *c, void *id, ...) {
    (void)e; (void)c;
    struct mono_va_wrap w;
    va_start(w.ap, id);
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE);
    (void)fn('V', 0, id, &w, 0);
    va_end(w.ap);
}
void mono_jni_st_V_v(void *e, void *c, void *id, va_list args) {
    (void)e; (void)c;
    struct mono_va_wrap w;
    va_copy(w.ap, args);
    mono_docall_fn fn = __atomic_load_n(&cb_doCall, __ATOMIC_ACQUIRE);
    (void)fn('V', 0, id, &w, 0);
    va_end(w.ap);
}

void *mono_jni_newobj_plain(void *e, void *c, void *id, ...) {
    (void)e;
    struct mono_va_wrap w;
    va_start(w.ap, id);
    mono_newobj_fn fn = __atomic_load_n(&cb_newObj, __ATOMIC_ACQUIRE);
    void *r = fn(c, id, &w);
    va_end(w.ap);
    return r;
}
void *mono_jni_newobj_v(void *e, void *c, void *id, va_list args) {
    (void)e;
    struct mono_va_wrap w;
    va_copy(w.ap, args);
    mono_newobj_fn fn = __atomic_load_n(&cb_newObj, __ATOMIC_ACQUIRE);
    void *r = fn(c, id, &w);
    va_end(w.ap);
    return r;
}
