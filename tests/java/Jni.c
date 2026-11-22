#include <jni.h>
#include <stdio.h>
#include <string.h>

JNIEXPORT jint JNICALL Java_Jni_add(JNIEnv *env, jclass c, jint a, jint b) { return a + b; }
JNIEXPORT jlong JNICALL Java_Jni_mul(JNIEnv *env, jclass c, jlong a, jdouble b) { return (jlong)(a * b); }
JNIEXPORT jstring JNICALL Java_Jni_greet(JNIEnv *env, jclass c, jstring who) {
    const char *w = (*env)->GetStringUTFChars(env, who, NULL);
    char buf[256];
    snprintf(buf, sizeof buf, "merhaba %s (%d)", w, (int)(*env)->GetStringLength(env, who));
    (*env)->ReleaseStringUTFChars(env, who, w);
    return (*env)->NewStringUTF(env, buf);
}
JNIEXPORT jint JNICALL Java_Jni_sum(JNIEnv *env, jclass c, jintArray a) {
    jsize n = (*env)->GetArrayLength(env, a);
    jint *p = (*env)->GetIntArrayElements(env, a, NULL);
    jint s = 0;
    for (int i = 0; i < n; i++) s += p[i];
    (*env)->ReleaseIntArrayElements(env, a, p, JNI_ABORT);
    return s;
}
JNIEXPORT void JNICALL Java_Jni_fill(JNIEnv *env, jclass c, jbyteArray a, jbyte v) {
    jsize n = (*env)->GetArrayLength(env, a);
    jbyte *p = (*env)->GetPrimitiveArrayCritical(env, a, NULL);
    memset(p, v, n);
    (*env)->ReleasePrimitiveArrayCritical(env, a, p, 0);
}
JNIEXPORT void JNICALL Java_Jni_fail(JNIEnv *env, jclass c, jstring msg) {
    jclass ex = (*env)->FindClass(env, "java/lang/IllegalStateException");
    const char *m = (*env)->GetStringUTFChars(env, msg, NULL);
    (*env)->ThrowNew(env, ex, m);
    (*env)->ReleaseStringUTFChars(env, msg, m);
}
JNIEXPORT jboolean JNICALL Java_Jni_isString(JNIEnv *env, jclass c, jobject o) {
    return (*env)->IsInstanceOf(env, o, (*env)->FindClass(env, "java/lang/String"));
}
JNIEXPORT jlong JNICALL Java_Jni_addressOf(JNIEnv *env, jclass c, jobject buf) {
    return (jlong)(*env)->GetDirectBufferAddress(env, buf) != 0 && (*env)->GetDirectBufferCapacity(env, buf) == 16;
}
JNIEXPORT jint JNICALL Java_Jni_countGlobal(JNIEnv *env, jclass c, jobject o) {
    jobject g = (*env)->NewGlobalRef(env, o);
    jint same = (*env)->IsSameObject(env, g, o);
    (*env)->DeleteGlobalRef(env, g);
    return same + ((*env)->ExceptionCheck(env) ? 10 : 0);
}
JNIEXPORT jint JNICALL Java_Jni_instanceAdd(JNIEnv *env, jobject self, jint x) {
    jclass k = (*env)->GetObjectClass(env, self);
    return x + ((*env)->IsInstanceOf(env, self, k) ? 100 : 0);
}

JNIEXPORT jstring JNICALL Java_Jni_callbacks(JNIEnv *env, jclass c, jobject self) {
    char buf[512];
    jclass k = (*env)->GetObjectClass(env, self);
    jmethodID twice = (*env)->GetMethodID(env, k, "twice", "(I)I");
    jint t = (*env)->CallIntMethod(env, self, twice, 21);
    jmethodID sj = (*env)->GetStaticMethodID(env, k, "sjoin", "(JDFZLjava/lang/Object;BCS)Ljava/lang/String;");
    jstring r = (jstring)(*env)->CallStaticObjectMethod(env, k, sj, (jlong)1 << 40, 2.5, 1.5f, JNI_TRUE, self == NULL ? NULL : (*env)->NewStringUTF(env, "obj"), (jbyte)-3, (jchar)'z', (jshort)-7);
    if ((*env)->ExceptionCheck(env)) {
        jthrowable ex = (*env)->ExceptionOccurred(env);
        (*env)->ExceptionClear(env);
        (*env)->CallStaticVoidMethod(env, k, (*env)->GetStaticMethodID(env, k, "dump", "(Ljava/lang/Throwable;)V"), ex);
        jmethodID ets = (*env)->GetMethodID(env, (*env)->GetObjectClass(env, ex), "toString", "()Ljava/lang/String;");
        return (jstring)(*env)->CallObjectMethod(env, ex, ets);
    }
    const char *rs = (*env)->GetStringUTFChars(env, r, NULL);
    jfieldID bf = (*env)->GetFieldID(env, k, "base", "I");
    jint b0 = (*env)->GetIntField(env, self, bf);
    (*env)->SetIntField(env, self, bf, b0 + 5);
    jint b1 = (*env)->GetIntField(env, self, bf);
    jfieldID sf = (*env)->GetStaticFieldID(env, k, "sfield", "Ljava/lang/String;");
    jstring sv = (jstring)(*env)->GetStaticObjectField(env, k, sf);
    const char *svs = (*env)->GetStringUTFChars(env, sv, NULL);
    jclass pk = (*env)->FindClass(env, "Jni$Pair");
    jmethodID pc = (*env)->GetMethodID(env, pk, "<init>", "(ILjava/lang/String;)V");
    jobject pair = (*env)->NewObject(env, pk, pc, 9, sv);
    jmethodID ts = (*env)->GetMethodID(env, pk, "toString", "()Ljava/lang/String;");
    jvalue none[1];
    jstring ps = (jstring)(*env)->CallObjectMethodA(env, pair, ts, none);
    const char *pss = (*env)->GetStringUTFChars(env, ps, NULL);
    jmethodID boom = (*env)->GetStaticMethodID(env, k, "boom", "()V");
    (*env)->CallStaticVoidMethod(env, k, boom);
    jboolean pending = (*env)->ExceptionCheck(env);
    (*env)->ExceptionClear(env);
    snprintf(buf, sizeof buf, "%d %s %d->%d %s %s pending=%d", t, rs, b0, b1, svs, pss, pending);
    return (*env)->NewStringUTF(env, buf);
}
JNIEXPORT jobject JNICALL Java_Jni_wrap(JNIEnv *env, jclass c) {
    static unsigned char mem[8] = {0, 1, 2, 3, 4, 5, 6, 7};
    return (*env)->NewDirectByteBuffer(env, mem, 8);
}

static jint registered_triple(JNIEnv *env, jclass c, jint x) { return x * 3; }
JNIEXPORT jint JNICALL Java_Jni_registerTriple(JNIEnv *env, jclass c) {
    JNINativeMethod m = { "triple", "(I)I", (void *)registered_triple };
    return (*env)->RegisterNatives(env, c, &m, 1);
}
