#!/usr/bin/env python3
"""Generate java/lang/MonoJavaLangAccess.java for the selected JDK (default output:
classlib/src/java/lang; monolith passes a per-JDK generated-source directory).

jdk.internal.access.JavaLangAccess changes between JDK releases, so the implementation is
generated from `javap` output of the JDK in use. Methods the native image can support are
implemented (IMPL, keyed by name, using the parameter names in PARAMS); every other method
throws UnsupportedOperationException naming the operation (never a silent fake result).
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from monolith.classpath import jdk_tool  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                   "classlib", "src", "java", "lang", "MonoJavaLangAccess.java")

IMPL = {
    "getEnumConstantsShared": "return klass.getEnumConstantsShared();",
    "registerShutdownHook": "Runtime.getRuntime().addShutdownHook(new Thread(hook));",
    "newThreadWithAcc": "return new Thread(target);",
    "invokeFinalize": "",
    "findBootstrapClassOrNull": "try { return Class.forName(name, false, null); } catch (ClassNotFoundException e) { return null; }",
    "fastUUID": "return hex(msb >> 32, 8) + \"-\" + hex(msb >> 16, 4) + \"-\" + hex(msb, 4) + \"-\" + hex(lsb >> 48, 4) + \"-\" + hex(lsb, 12);",
    "countPositives": "int i = off; for (; i < off + len; i++) if (ba[i] < 0) break; return i - off;",
    "uncheckedCountPositives": "int i = off; for (; i < off + len; i++) if (ba[i] < 0) break; return i - off;",
    "countNonZeroAscii": "int n = 0; for (int i = 0; i < s.length(); i++) { char c = s.charAt(i); if (c != 0 && c < 0x80) n++; else break; } return n;",
    "newStringNoRepl": "return new String(bytes, cs);",
    "uncheckedNewStringNoRepl": "return new String(bytes, cs);",
    "getBytesNoRepl": "return s.getBytes(cs);",
    "uncheckedGetBytesNoRepl": "return s.getBytes(cs);",
    "newStringUTF8NoRepl": "return new String(bytes, off, len);",
    "getUTF16Char": "return (char) (((bytes[index << 1] & 0xff) << 8) | (bytes[(index << 1) + 1] & 0xff));",
    "uncheckedGetUTF16Char": "return (char) (((bytes[index << 1] & 0xff) << 8) | (bytes[(index << 1) + 1] & 0xff));",
    "uncheckedPutCharUTF16": "bytes[index << 1] = (byte) (ch >> 8); bytes[(index << 1) + 1] = (byte) ch;",
    "getBytesUTF8NoRepl": "return s.getBytes();",
    "inflateBytesToChars": "for (int i = 0; i < len; i++) dst[dstOff + i] = (char) (src[srcOff + i] & 0xff);",
    "uncheckedInflateBytesToChars": "for (int i = 0; i < len; i++) dst[dstOff + i] = (char) (src[srcOff + i] & 0xff);",
    "decodeASCII": "int i = 0; for (; i < len; i++) { byte b = src[srcOff + i]; if (b < 0) break; dst[dstOff + i] = (char) b; } return i;",
    "uncheckedDecodeASCII": "int i = 0; for (; i < len; i++) { byte b = src[srcOff + i]; if (b < 0) break; dst[dstOff + i] = (char) b; } return i;",
    "encodeASCII": "int i = 0; for (; i < len; i++) { char c = src[srcOff + i]; if (c >= 0x80) break; dst[dstOff + i] = (byte) c; } return i;",
    "uncheckedEncodeASCII": "int i = 0; for (; i < len; i++) { char c = src[srcOff + i]; if (c >= 0x80) break; dst[dstOff + i] = (byte) c; } return i;",
    "bytesCompatible": "String n = cs.name(); for (int i = 0; i < s.length(); i++) if (s.charAt(i) >= 0x80) return false; return n.equals(\"UTF-8\") || n.equals(\"ISO-8859-1\") || n.equals(\"US-ASCII\");",
    "copyToSegmentRaw": "throw new UnsupportedOperationException(\"monolith: JavaLangAccess.copyToSegmentRaw\");",
    "initialSystemIn": "return System.in;",
    "initialSystemErr": "return System.err;",
    "setCause": "t.initCause(cause);",
    "join": "StringBuilder sb = new StringBuilder(prefix); for (int i = 0; i < size; i++) { if (i > 0) sb.append(delimiter); sb.append(elements[i]); } return sb.append(suffix).toString();",
    "concat": "return String.valueOf(prefix) + value + String.valueOf(suffix);",
    "uncheckedStringConcat1": "StringBuilder sb = new StringBuilder(); for (int i = 0; i < constants.length; i++) { sb.append(constants[i]); } return sb.toString();",
    "exit": "Runtime.getRuntime().exit(statusCode);",
    "getAllThreads": "return new Thread[]{Thread.currentThread()};",
    "threadContainer": "return null;",
    "start": "thread.start();",
    "headStackableScope": "return null;",
    "setHeadStackableScope": "",
    "currentCarrierThread": "return Thread.currentThread();",
    "executeOnCarrierThread": "return task.call();",
    "getCarrierThreadLocal": "return ((ThreadLocal<T>) local).getCarrierThreadLocal();",
    "setCarrierThreadLocal": "((ThreadLocal<T>) local).setCarrierThreadLocal(value);",
    "removeCarrierThreadLocal": "((ThreadLocal<?>) local).removeCarrierThreadLocal();",
    "isCarrierThreadLocalPresent": "return ((ThreadLocal<?>) local).isCarrierThreadLocalPresent();",
    "scopedValueCache": "return null;",
    "setScopedValueCache": "",
    "scopedValueBindings": "return null;",
    "allowSecurityManager": "return false;",
    "getLoaderNameID": "return loader == null ? \"bootstrap\" : String.valueOf(loader.getName());",
    "invalidatePackageAccessCache": "",
    "protectionDomain": "return null;",
    "getDeclaredAnnotationMap": "return klass.getDeclaredAnnotationMap();",
    "getRawClassAnnotations": "return klass.getRawAnnotations();",
    "getConstantPool": "return klass.getConstantPool();",
    "casAnnotationType": "return klass.casAnnotationType(oldType, newType);",
    "getAnnotationType": "return klass.getAnnotationType();",
    "getRawClassTypeAnnotations": "return null;",
    "getRawExecutableTypeAnnotations": "return null;",
    "classFileVersion": "return 69;",
    "blockedOn": "Thread.blockedOn(b);",
}
PARAMS = {
    "blockedOn": ["b"],
    "getDeclaredAnnotationMap": ["klass"], "getRawClassAnnotations": ["klass"], "getConstantPool": ["klass"],
    "casAnnotationType": ["klass", "oldType", "newType"], "getAnnotationType": ["klass"],
    "getEnumConstantsShared": ["klass"], "registerShutdownHook": ["slot", "registerShutdownInProgress", "hook"],
    "newThreadWithAcc": ["target", "acc"], "findBootstrapClassOrNull": ["name"], "fastUUID": ["lsb", "msb"],
    "countPositives": ["ba", "off", "len"], "uncheckedCountPositives": ["ba", "off", "len"],
    "countNonZeroAscii": ["s"], "newStringNoRepl": ["bytes", "cs"], "uncheckedNewStringNoRepl": ["bytes", "cs"],
    "getBytesNoRepl": ["s", "cs"], "uncheckedGetBytesNoRepl": ["s", "cs"],
    "newStringUTF8NoRepl": ["bytes", "off", "len"], "getUTF16Char": ["bytes", "index"],
    "uncheckedGetUTF16Char": ["bytes", "index"], "uncheckedPutCharUTF16": ["bytes", "index", "ch"],
    "getBytesUTF8NoRepl": ["s"], "bytesCompatible": ["s", "cs"],
    "inflateBytesToChars": ["src", "srcOff", "dst", "dstOff", "len"],
    "uncheckedInflateBytesToChars": ["src", "srcOff", "dst", "dstOff", "len"],
    "decodeASCII": ["src", "srcOff", "dst", "dstOff", "len"], "uncheckedDecodeASCII": ["src", "srcOff", "dst", "dstOff", "len"],
    "encodeASCII": ["src", "srcOff", "dst", "dstOff", "len"], "uncheckedEncodeASCII": ["src", "srcOff", "dst", "dstOff", "len"],
    "setCause": ["t", "cause"], "join": ["prefix", "suffix", "delimiter", "elements", "size"],
    "concat": ["prefix", "value", "suffix"], "uncheckedStringConcat1": ["constants"],
    "exit": ["statusCode"], "start": ["thread", "container"], "executeOnCarrierThread": ["task"],
    "getCarrierThreadLocal": ["local"], "setCarrierThreadLocal": ["local", "value"],
    "removeCarrierThreadLocal": ["local"], "isCarrierThreadLocalPresent": ["local"], "getLoaderNameID": ["loader"],
}


def split_params(params: str) -> list[str]:
    ps, depth, cur = [], 0, ""
    for ch in params:
        if ch == "<":
            depth += 1
        if ch == ">":
            depth -= 1
        if ch == "," and depth == 0:
            ps.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        ps.append(cur.strip())
    return ps


def main() -> int:
    global OUT
    if len(sys.argv) > 1:
        OUT = sys.argv[1]
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    env = {k: v for k, v in os.environ.items() if k != "JAVA_TOOL_OPTIONS"}
    out = subprocess.run([jdk_tool("javap"), "-J-Xmx128m", "-J-XX:+UseSerialGC",
                          "-p", "jdk.internal.access.JavaLangAccess"],
                         capture_output=True, text=True, env=env, check=True).stdout
    sigs = [l.strip().replace("public abstract ", "") for l in out.splitlines() if "(" in l]
    lines = ["package java.lang;\n\n",
             "/** monolith: JavaLangAccess for the native image. GENERATED by tools/gen_jla.py from the\n",
             " * selected JDK — do not edit. Unsupported operations throw UnsupportedOperationException. */\n",
             "final class MonoJavaLangAccess implements jdk.internal.access.JavaLangAccess {\n"]
    for sig in sigs:
        m = re.match(r"(?:(<[^>]+>) )?(.+?) (\w+)\((.*)\)( throws .*)?;$", sig)
        if not m:
            continue
        tparams, ret, name, params, throws = m.groups()
        ps = split_params(params)
        names = PARAMS.get(name, [f"a{i}" for i in range(len(ps))])
        if len(names) != len(ps):
            names = [f"a{i}" for i in range(len(ps))]
        plist = []
        for t, n in zip(ps, names):
            t = t.replace("$", ".")
            if t.endswith("..."):
                t = t[:-3] + "[]"
            plist.append(f"{t} {n}")
        body = IMPL.get(name)
        if body is None or (name == "start" and len(ps) != 2) or (name == "concat" and len(ps) != 3):
            body = f'throw new UnsupportedOperationException("monolith: JavaLangAccess.{name}");'
        tp = (tparams + " ") if tparams else ""
        lines.append(f"    public {tp}{ret.replace('$', '.')} {name}({', '.join(plist)}){throws or ''} {{ {body} }}\n")
    lines.append('''
    private static String hex(long v, int digits) {
        long hi = 1L << (digits * 4);
        return Long.toHexString(hi | (v & (hi - 1))).substring(1);
    }
}
''')
    new = "".join(lines)
    old = open(OUT).read() if os.path.exists(OUT) else ""
    if new != old:
        with open(OUT, "w") as fh:
            fh.write(new)
    return 0


if __name__ == "__main__":
    sys.exit(main())
