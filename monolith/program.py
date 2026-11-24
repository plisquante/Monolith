"""Whole-program view: instruction preparation (indy desugaring) and Rapid Type Analysis.

RTA keeps the generated program to the methods that can actually execute, which is
what makes transpiling an application together with its slice of the class library
tractable (the JDK alone has >20k classes).
"""
from __future__ import annotations

import collections
import os
from typing import Optional

from .proxygen import make_mh_invoker, make_proxy
from .reflectmeta import java_string_hash, proxy_class_name

from .bytecode import UNSUPPORTED, Insn, decode
from .classfile import CONSTANT_Methodref, ClassConst, MemberRef, MethodInfo, DynamicConst
from .indy import IndyDesugarer
from .world import ClassInfo, LinkError, World, parse_method_desc

# Çalışma zamanının kendisinin fırlattığı Throwable'lar (çalışma zamanındaki istisna enum sırasıyla eşleşmek zorunda).
RT_EXCEPTIONS = [
    ("NPE", "java/lang/NullPointerException"),
    ("Arithmetic", "java/lang/ArithmeticException"),
    ("ArrayIndex", "java/lang/ArrayIndexOutOfBoundsException"),
    ("ClassCast", "java/lang/ClassCastException"),
    ("NegativeArraySize", "java/lang/NegativeArraySizeException"),
    ("ArrayStore", "java/lang/ArrayStoreException"),
    ("OutOfMemory", "java/lang/OutOfMemoryError"),
    ("IllegalMonitorState", "java/lang/IllegalMonitorStateException"),
    ("UnsatisfiedLink", "java/lang/UnsatisfiedLinkError"),
    ("NoClassDefFound", "java/lang/NoClassDefFoundError"),
    ("AbstractMethod", "java/lang/AbstractMethodError"),
    ("Interrupted", "java/lang/InterruptedException"),
    ("CloneNotSupported", "java/lang/CloneNotSupportedException"),
    ("StackOverflow", "java/lang/StackOverflowError"),
    ("UnsupportedOperation", "java/lang/UnsupportedOperationException"),
    ("IncompatibleClassChange", "java/lang/IncompatibleClassChangeError"),
    ("NoSuchField", "java/lang/NoSuchFieldError"),
    ("NoSuchMethod", "java/lang/NoSuchMethodError"),
    ("IllegalArgument", "java/lang/IllegalArgumentException"),
    ("InternalError", "java/lang/InternalError"),
    ("IllegalThreadState", "java/lang/IllegalThreadStateException"),
    ("Instantiation", "java/lang/InstantiationError"),
    ("IO", "java/io/IOException"),
    ("FileNotFound", "java/io/FileNotFoundException"),
    ("OutOfMemoryNative", "java/lang/OutOfMemoryError"),
]

# Çalışma zamanının doğrudan çağırdığı Java metotları (RTA'dan sağ çıkmak zorunda).
RT_STATIC_ROOTS = [
    ("java/nio/DirectByteBuffer", "<init>", "(JJ)V"),  # JNI NewDirectByteBuffer
    # yansıtmalı çağrıların kullandığı kutulama (monolith/rt/natives_reflect.rs)
    ("java/lang/Boolean", "valueOf", "(Z)Ljava/lang/Boolean;"), ("java/lang/Byte", "valueOf", "(B)Ljava/lang/Byte;"),
    ("java/lang/Character", "valueOf", "(C)Ljava/lang/Character;"), ("java/lang/Short", "valueOf", "(S)Ljava/lang/Short;"),
    ("java/lang/Integer", "valueOf", "(I)Ljava/lang/Integer;"), ("java/lang/Long", "valueOf", "(J)Ljava/lang/Long;"),
    ("java/lang/Float", "valueOf", "(F)Ljava/lang/Float;"), ("java/lang/Double", "valueOf", "(D)Ljava/lang/Double;"),
    ("java/lang/Thread", "monoForeign", "()Ljava/lang/Thread;"),
    ("java/lang/Throwable", "monoNcdfe", "(Ljava/lang/Class;Ljava/lang/Throwable;Ljava/lang/Thread;)Ljava/lang/Throwable;"),
    ("java/lang/ExceptionInInitializerError", "<init>", "(Ljava/lang/Throwable;)V"),
    ("java/lang/Thread", "monoStartMain", "()Ljava/lang/Thread;"),
    ("java/lang/Thread", "monoRun", "(Ljava/lang/Thread;)V"),
    ("java/lang/Thread", "monoUncaught", "(Ljava/lang/Thread;Ljava/lang/Throwable;)V"),
    ("java/lang/System", "monoShutdown", "()V"),
]
RT_VIRTUAL_ROOTS = [
    ("java/lang/ref/Reference", "enqueue", "()Z"),
    ("java/lang/Runnable", "run", "()V"),
    ("java/lang/Object", "toString", "()Ljava/lang/String;"),
]
RT_INSTANTIATED = ["java/lang/Object", "java/lang/String", "java/lang/Class",
                   "java/lang/StackTraceElement"]
RT_LOADED = ["java/lang/Cloneable", "java/io/Serializable", "java/lang/Enum"]


# Bu sınıflardan nesne oluşturan yerel metotlar (RTA yerel kodun içini göremez).
NATIVE_INSTANTIATES = {
    ("java/lang/Class", "newField0(Ljava/lang/String;)Ljava/lang/reflect/Field;"): ["java/lang/reflect/Field"],
    ("java/lang/Class", "fields0()[Ljava/lang/Object;"): ["java/lang/reflect/Field"],
    ("java/lang/Class", "methods0(Z)[Ljava/lang/Object;"): ["java/lang/reflect/Method", "java/lang/reflect/Constructor"],
    ("java/lang/Class", "getConstantPool()Ljdk/internal/reflect/ConstantPool;"): ["jdk/internal/reflect/ConstantPool"],
    ("sun/management/MemoryImpl", "getMemoryUsage0(Z)Ljava/lang/management/MemoryUsage;"): ["java/lang/management/MemoryUsage"],
}


# module-info sağlayıcıları, servis arayüzüne erişilebildiğinde bağlanan JDK servisleri
# (her servisin her sağlayıcısı JDK'nın çoğunu içeri çekerdi).
JDK_SERVICES = {"java/nio/file/spi/FileSystemProvider"}

# JDK'nın çalışma zamanında adıyla kendisinin örneklediği JDK sınıfları (yerel ayar sağlayıcıları, kaynak
# paketleri, güvenlik sağlayıcıları, karakter kümeleri): giriş noktalarına erişilebildiğinde tutulur.
JDK_REFLECT_RULES = [
    {"trigger": "sun/util/locale/provider/LocaleProviderAdapter",
     "classes": ["sun/util/locale/provider/JRELocaleProviderAdapter",
                 "sun/util/locale/provider/FallbackLocaleProviderAdapter",
                 "sun/text/resources/FormatData", "sun/text/resources/FormatData_en",
                 "sun/util/resources/CalendarData", "sun/util/resources/CalendarData_en",
                 # CLDR, varsayılan sağlayıcı: bağdaştırıcısı ve adıyla yüklediği java.base paketleri
                 # (dünya listesindeki yerelleştirilmiş java.time tarih biçimleri bunlar olmadan başarısız oluyordu)
                 "sun/util/cldr/CLDRLocaleProviderAdapter", "sun/util/cldr/CLDRBaseLocaleDataMetaInfo",
                 "sun/text/resources/cldr/FormatData", "sun/util/resources/cldr/CalendarData",
                 "sun/util/resources/cldr/CurrencyNames", "sun/util/resources/cldr/LocaleNames",
                 "sun/util/resources/cldr/TimeZoneNames"]},
    # URL.DefaultFactory, http/https akış işleyicilerini adıyla yükler (Class.forName)
    {"trigger": "java/net/HttpURLConnection",
     "classes": ["sun/net/www/protocol/http/Handler", "sun/net/www/protocol/https/Handler"]},
    {"trigger": "sun/nio/cs/StandardCharsets",
     "classes": ["sun/nio/cs/UTF_8", "sun/nio/cs/ISO_8859_1", "sun/nio/cs/US_ASCII",
                 "sun/nio/cs/UTF_16", "sun/nio/cs/UTF_16BE", "sun/nio/cs/UTF_16LE",
                 "sun/nio/cs/UTF_32", "sun/nio/cs/UTF_32BE", "sun/nio/cs/UTF_32LE",
                 "sun/nio/cs/CESU_8", "sun/nio/cs/UnicodeBig", "sun/nio/cs/UnicodeLittle"]},
]
# dize sabitleri sağlayıcı uygulamalarını adlandıran sınıflar (Provider.putService kayıtları)
JDK_STRING_REGISTRIES = [
    "sun/security/provider/SunEntries", "sun/security/rsa/SunRsaSignEntries", "com/sun/crypto/provider/SunJCE",
    "sun/security/ssl/SunJSSE", "sun/security/ec/SunEC", "sun/security/provider/certpath/CertPathProvider",
    "sun/security/jca/ProviderConfig",
]

# Bu sınıfların yerel metotları (soketler, ad çözümleme, arayüzler) Java nesneleri kurar ve JDK'nın soket
# istisnalarını yerel koddan fırlatır: bu sınıfları ve yardımcı metotları tut.
NET_NATIVE_PREFIXES = ("sun/nio/ch/", "java/net/", "sun/nio/fs/LinuxWatchService")
NET_CLASSES = [
    "java/net/ConnectException", "java/net/BindException", "java/net/NoRouteToHostException",
    "java/net/SocketException", "java/net/SocketTimeoutException", "java/net/UnknownHostException",
    "java/net/PortUnreachableException", "java/net/ProtocolException", "java/net/Inet4Address",
    "java/net/Inet6Address", "java/net/InetSocketAddress", "java/net/InterfaceAddress",
    "java/net/NetworkInterface", "java/net/InetAddress$InetAddressHolder",
    "java/net/Inet6Address$Inet6AddressHolder",
]
NET_METHODS = [
    ("java/net/InetAddress", "getByAddress", "(Ljava/lang/String;[B)Ljava/net/InetAddress;"),
    ("java/net/Inet6Address", "getByAddress", "(Ljava/lang/String;[BI)Ljava/net/Inet6Address;"),
    ("java/net/InetSocketAddress", "<init>", "(Ljava/net/InetAddress;I)V"),
    # DatagramSocketAdaptor.joinGroup (çok oyunculu ekrandaki Minecraft LAN algılaması) "herhangi" arayüzü
    # bu kurucu üzerindeki bir method handle ile kurar
    ("java/net/NetworkInterface", "<init>", "(Ljava/lang/String;I[Ljava/net/InetAddress;)V"),
]


# Bir sınıfın yerel metotlarının geri çağırdığı Java kodu: sınıf -> [(sınıf, metot, tanımlayıcı)].
NATIVE_CALLBACKS = {
    "sun/nio/fs/UnixNativeDispatcher": [("sun/nio/fs/UnixException", "<init>", "(I)V")],
}

# Yalnızca uygulama belirli bir kütüphaneyi kullandığında var olan kökler (yerel koddan çağrılır).
OPTIONAL_VIRTUAL_ROOTS = [
    ("org/lwjgl/system/CallbackI", "callback", "(JJ)V"),   # LWJGL upcalls (libffi closures)
]


class Report:
    def __init__(self):
        self.items: dict[str, list[str]] = collections.defaultdict(list)

    def __call__(self, kind: str, msg: str):
        self.items[kind].append(msg)


class Program:
    def __init__(self, world: World, report: Optional[Report] = None):
        self.w = world
        self.report = report or Report()
        self.indy = IndyDesugarer(self.report)
        self._insns: dict[tuple[str, str], list[Insn]] = {}
        self.reachable: set[tuple[str, str]] = set()   # (class, name+desc)
        self.instantiated: set[str] = set()
        self.skip_scan: set = set()   # (class, method, desc) whose body is not analysed (patches with "replace": true)
        self.initialized: set[str] = set()
        self.loaded: set[str] = set()
        self.vcalls: dict[str, set[str]] = collections.defaultdict(set)  # key -> owners
        self.work: collections.deque = collections.deque()
        self.natives: set[tuple[str, str]] = set()
        self.enum_classes: set[str] = set()
        # yansıma politikası: tüm metot/kurucuları tutulan sınıf adı önekleri
        self.reflect_prefixes: list[str] = []
        self._reflected: set[str] = set()
        self.proxies: set[str] = set()
        # sabit adla, sabit olmayan bir alıcı sınıf üzerinde yansıtmalı aramalar
        # (parametre olarak geçirilen Class<T>): bir `X.class` ile adlandırılan her sınıfa karşı çözülür
        self.dyn_names: set[str] = set()
        self.ldc_classes: set[str] = set()
        self._dyn_done: set[tuple[str, str]] = set()
        self.native_libs: list[str] = []
        self._net_roots = False
        self._mh_stubs: dict[str, MemberRef] = {}
        self.reflect_classes: list[str] = []
        self.reflected_seen: set[str] = set()  # classes kept for reflection (registries, rules); the back end must emit them
        self._annotations_seen: set[str] = set()
        self.why: dict[tuple[str, str], tuple[str, str]] = {}
        self._cur: tuple[str, str] = ("<root>", "")

    # ------------------------------------------------------------- komutlar
    def insns(self, ci: ClassInfo, m: MethodInfo) -> list[Insn]:
        k = (ci.name, m.key)
        r = self._insns.get(k)
        if r is not None:
            return r
        pre = getattr(m, "insns", None)
        if pre is not None:
            r = list(pre)
        else:
            r = decode(m.code.code, ci.cf.cp)
            for i, ins in enumerate(r):
                if ins.op == "invokedynamic":
                    r[i] = self.indy.desugar(ci.cf, m, ins)
                    pending, self.indy.new_classes = self.indy.new_classes, []
                    for ncf in pending:
                        try:
                            self.w.add_synthetic(ncf)
                        except LinkError as e:
                            # örn. sınıf yolunda olmayan isteğe bağlı bir bağımlılığın arayüzünü uygulayan bir lambda:
                            # çağrı noktası çalışma zamanında başarısız olur
                            # (NoClassDefFoundError), JVM'in başarısız olacağı yerde tam olarak.
                            self.report("link-error", f"{ci.name}.{m.name}{m.desc}: {e}")
                elif ins.op == "invokevirtual" and ins.arg.owner == "java/lang/invoke/MethodHandle" \
                        and ins.arg.name in ("invoke", "invokeExact", "invokeBasic"):
                    r[i] = Insn(ins.pc, "invokestatic", self.mh_invoker(ins.arg.desc), None, (), ins.length)
                elif ins.op in UNSUPPORTED:
                    self.report("unsupported-insn", f"{ci.name}.{m.name}{m.desc}@{ins.pc}: {ins.op}")
                elif ins.op == "ldc" and isinstance(ins.arg, DynamicConst):
                    self.report("unsupported-condy", f"{ci.name}.{m.name}{m.desc}@{ins.pc}")
        # Assertion'lar kapalıdır (Class.desiredAssertionStatus() == false, JVM varsayılanı), bu yüzden
        # `getstatic $assertionsDisabled; ifne L` her zaman atlar: RTA assertion mesajı kod yollarını içeri
        # çekmesin diye bunu katla.
        for i in range(len(r) - 1):
            a, b = r[i], r[i + 1]
            if a.op == "getstatic" and a.arg.name == "$assertionsDisabled" and a.arg.desc == "Z" \
                    and b.op == "ifne":
                r[i] = Insn(a.pc, "nop", None, None, (), a.length)
                r[i + 1] = Insn(b.pc, "goto", None, None, b.targets, b.length)
        self._insns[k] = r
        return r

    # ------------------------------------------------------------- RTA (hızlı tür analizi)
    def load(self, name: str) -> Optional[ClassInfo]:
        while name.startswith("["):
            name = name[1:]
        if name.startswith("L") and name.endswith(";"):
            name = name[1:-1]
        if len(name) == 1:
            return None
        try:
            ci = self.w.get(name)
        except LinkError as e:
            self.report("link-error", str(e))
            return None
        if ci is None:
            self.report("missing-class", name)
            return None
        if name not in self.loaded:
            self.loaded.add(name)
            for s in ci.supers():
                self.load(s.name)
            for i in ci.interfaces:
                self.load(i.name)
            # alan türlerine yerleşimler tarafından başvurulur
        return ci

    def init_class(self, name: str):
        if name in self.initialized:
            return
        ci = self.load(name)
        if ci is None:
            return
        self.initialized.add(name)
        if ci.super and ci.super.name == "java/lang/Enum" and name not in self.enum_classes:
            # sabite özgü gövdeli enum sınıfları kendileri hiç örneklenmez,
            # ama Enum.valueOf / getEnumConstants onların values() metoduna ihtiyaç duyar
            self.enum_classes.add(name)
            vm = ci.cf.method("values", f"()[L{name};")
            if vm is not None:
                self.mark(ci, vm)
        if ci.super:
            self.init_class(ci.super.name)
        m = ci.cf.method("<clinit>", "()V")
        if m:
            self.mark(ci, m)
        for f in ci.cf.fields:
            if f.is_static and f.desc == "Ljava/lang/String;" and f.constant_value is not None:
                self.instantiate("java/lang/String")

    def net_roots(self):
        for n in NET_CLASSES:
            if self.w.cp.find(n) is not None:
                self.instantiate(n)
        for o, n, d in NET_METHODS:
            ci = self.load(o)
            m = ci.cf.method(n, d) if ci else None
            if m is not None:
                if n == "<init>":
                    self.instantiate(o)
                else:
                    self.init_class(o)
                self.mark(ci, m)

    def mark(self, ci: ClassInfo, m: MethodInfo):
        k = (ci.name, m.key)
        if k in self.reachable:
            return
        self.reachable.add(k)
        self.why[k] = self._cur
        if m.is_native:
            self.natives.add(k)
            if not self._net_roots and ci.name.startswith(NET_NATIVE_PREFIXES):
                self._net_roots = True
                self.net_roots()
            for c in NATIVE_INSTANTIATES.get(k, ()):
                self.instantiate(c)
            for oc, on, od in NATIVE_CALLBACKS.get(ci.name, ()):
                tci = self.load(oc)
                tm = tci.cf.method(on, od) if tci else None
                if tm is not None:
                    if on == "<init>":
                        self.instantiate(oc)
                    self.mark(tci, tm)
        if m.code is not None:
            self.work.append((ci, m))

    def instantiate(self, name: str):
        if name in self.instantiated:
            return
        ci = self.load(name)
        if ci is None or ci.is_interface:
            return
        self.instantiated.add(name)
        self.init_class(name)
        # JNI ThrowNew, yerel koddan örneklenmiş herhangi bir Throwable'ı kurabilir
        thr = self.w.classes.get("java/lang/Throwable")
        if thr is not None and self.w.is_subclass(ci, thr):
            sm = ci.cf.method("<init>", "(Ljava/lang/String;)V")
            if sm is not None:
                self.mark(ci, sm)
        if ci.cf.access & 0x4000 or (ci.super and ci.super.name == "java/lang/Enum"):
            self.enum_classes.add(name)
            vm = ci.cf.method("values", f"()[L{name};")
            if vm is not None:
                self.mark(ci, vm)
        vt = self.w.vtable(ci)
        for s in vt:
            key = s.name + s.desc
            owners = self.vcalls.get(key)
            if not owners or s.impl is None:
                continue
            for o in owners:
                oc = self.w.classes.get(o)
                if oc is not None and self.w.is_subclass(ci, oc):
                    impl_ci = self.w.classes[s.impl[0]]
                    self.mark(impl_ci, s.impl[1])
                    break

    def vcall(self, owner: str, name: str, desc: str):
        key = name + desc
        owners = self.vcalls[key]
        if owner in owners:
            return
        owners.add(owner)
        oc = self.w.classes.get(owner)
        if oc is None:
            return
        # yüklenmiş alt tür grafiğinde gez
        seen = set()
        stack = [oc]
        while stack:
            c = stack.pop()
            if c.name in seen:
                continue
            seen.add(c.name)
            if c.name in self.instantiated:
                sel = self.w.select(c, name, desc)
                if sel:
                    self.mark(sel[0], sel[1])
            stack.extend(c.subclasses)

    def resolve(self, ref: MemberRef):
        owner = ref.owner
        if owner.startswith("["):
            owner = "java/lang/Object"
        try:
            return self.w.resolve_method(owner, ref.name, ref.desc, ref.is_interface)
        except LinkError as e:
            self.report("link-error", str(e))
            return None

    def run(self, main_class: str, extra_roots: list[tuple[str, str, str]] = (), extra_vroots: list[tuple[str, str, str]] = ()):
        for n in RT_LOADED:
            self.load(n)
        for n in RT_INSTANTIATED:
            self.instantiate(n)
        for _, n in RT_EXCEPTIONS:
            self.instantiate(n)
            ci = self.w.classes.get(n)
            if ci:
                for d in ("()V", "(Ljava/lang/String;)V"):
                    m = ci.cf.method("<init>", d)
                    if m:
                        self.mark(ci, m)
        for o, n, d in list(RT_STATIC_ROOTS) + list(extra_roots):
            ci = self.load(o)
            if ci is None:
                continue
            m = ci.cf.method(n, d)
            if m is None:
                self.report("missing-root", f"{o}.{n}{d}")
                continue
            self.init_class(o)
            if n == "<init>":
                self.instantiate(o)
            self.mark(ci, m)
        for o, n, d in extra_vroots:
            if self.load(o):
                self.vcall(o, n, d)
        for o, n, d in RT_VIRTUAL_ROOTS + OPTIONAL_VIRTUAL_ROOTS:
            if (o, n, d) in OPTIONAL_VIRTUAL_ROOTS and self.w.cp.find(o) is None:
                continue
            if self.load(o):
                self.vcall(o, n, d)
        # uygulamanın META-INF/services dosyalarında bildirilen ServiceLoader sağlayıcıları
        self.services: dict[str, list[str]] = {}
        for svc, provs in self.w.cp.services().items():
            if self.w.get(svc) is None:
                continue
            for pv in provs:
                pci = self.w.get(pv)
                m = pci.cf.method("<init>", "()V") if pci is not None else None
                if m is None:
                    self.report("service-provider", f"{svc}: {pv} not found or has no no-arg constructor")
                    continue
                self.instantiate(pv)
                self.mark(pci, m)
                self.services.setdefault(svc, []).append(pv)
        self._module_services = {k: v for k, v in self.w.cp.module_services().items() if k in JDK_SERVICES}
        self._module_services_done: set[str] = set()
        self.service_factories: set[tuple[str, str]] = set()
        mc = self.load(main_class)
        if mc is None:
            raise LinkError(f"main class {main_class} not found")
        mm = mc.cf.method("main", "([Ljava/lang/String;)V")
        if mm is None:
            raise LinkError(f"{main_class}.main(String[]) not found")
        self.init_class(main_class)
        self.mark(mc, mm)
        self.native_lib_refs()
        for n in self.reflect_classes:
            if self.w.cp.find(n) is None:
                self.report("reflect-class", f"{n} not found")
                continue
            self.init_class(n)
            self.reflect_all(n)
        while True:
            while self.work:
                ci, m = self.work.popleft()
                self.scan(ci, m)
            self.jdk_reflect()
            for cn in sorted(self.ldc_classes):
                ci = self.w.classes.get(cn)
                if ci is None:
                    continue
                mn = self._method_names(ci)
                for n in sorted(self.dyn_names):
                    if (cn, n) in self._dyn_done:
                        continue
                    self._dyn_done.add((cn, n))
                    if n in mn:
                        c = ci
                        while c is not None:
                            self.reflect_all(c.name, n)
                            c = c.super
            for svc, provs in self._module_services.items():
                if svc in self.loaded and svc not in self._module_services_done:
                    self._module_services_done.add(svc)
                    self._cur = ("<root>", f"service:{svc}")
                    for pv, _ in provs:
                        pci = self.w.get(pv)
                        if pci is None:
                            continue
                        ctor = pci.cf.method("<init>", "()V")
                        if ctor is not None and ctor.access & 1:
                            self.instantiate(pv)
                            self.mark(pci, ctor)
                            self.services.setdefault(svc, []).append(pv)
                        else:
                            fm = pci.cf.method("provider", f"()L{svc};") or next(
                                (m for m in pci.cf.methods if m.name == "provider" and m.is_static
                                 and m.desc.startswith("()")), None)
                            if fm is None:
                                self.report("service-provider", f"{svc}: {pv} has neither a public constructor nor provider()")
                                continue
                            self.init_class(pv)
                            self.mark(pci, fm)
                            self.service_factories.add((svc, pv))
                            self.services.setdefault(svc, []).append(pv)
            todo = [n for n in list(self.loaded) if n not in self._reflected
                    and any(n.startswith(px) for px in self.reflect_prefixes)]
            for n in todo:
                self._reflected.add(n)
                self.reflect_all(n)
            # kapalı dünyadaki açıklama (annotation) arayüzleri vekil sınıflar alır (AnnotationParser)
            anns = [n for n in list(self.loaded) if n not in self._annotations_seen
                    and self.w.classes[n].cf.access & 0x2000]
            for n in anns:
                self._annotations_seen.add(n)
                self.annotation_type(n)
            if not todo and not anns and not self.work and not self._pending_dyn():
                break

    def mh_invoker(self, desc: str) -> MemberRef:
        stub = self._mh_stubs.get(desc)
        if stub is None:
            name = f"java/lang/invoke/MonoMHInvoke${java_string_hash(desc):08x}"
            while name in self.w.synthetic:
                name += "_"
            cf, sdesc = make_mh_invoker(name, desc)
            self.w.add_synthetic(cf)
            stub = self._mh_stubs[desc] = MemberRef(CONSTANT_Methodref, name, "invoke", sdesc)
        return stub

    def native_lib_refs(self):
        """Native libraries call back into Java by name (FindClass/GetMethodID/GetFieldID): keep
        every method of a class named inside the library whose name also occurs in the library."""
        import re
        tok = re.compile(rb"<?[A-Za-z_$][A-Za-z0-9_$]*>?")
        cls_re = re.compile(rb"[A-Za-z_$][A-Za-z0-9_$]*(?:/[A-Za-z0-9_$]+)+")
        for path in self.native_libs:
            try:
                data = open(path, "rb").read()
            except OSError:
                continue
            names = {m.decode() for m in cls_re.findall(data)}
            idents = {m.decode() for m in tok.findall(data)}
            for cn in sorted(names):
                if self.w.cp.find(cn) is None:
                    continue
                try:
                    ci = self.load(cn)
                except LinkError:
                    ci = None
                if ci is None:
                    continue
                self._cur = ("<root>", f"native-lib:{os.path.basename(path)}:{cn}")
                for m in ci.cf.methods:
                    if m.name in idents and m.name != "<clinit>":
                        self.reflect_all(cn, m.name)

    def _method_names(self, ci) -> set:
        cache = self.__dict__.setdefault("_mn_cache", {})
        r = cache.get(ci.name)
        if r is None:
            r = {m.name for c2 in [ci] + list(ci.supers()) for m in c2.cf.methods}
            cache[ci.name] = r
        return r

    def _pending_dyn(self) -> bool:
        return any((cn, n) not in self._dyn_done for cn in self.ldc_classes if cn in self.w.classes
                   for n in self.dyn_names)

    def jdk_reflect(self):
        import re
        for ri, rule in enumerate(JDK_REFLECT_RULES):
            key = f"rule{ri}"
            if key in self._reflected or rule["trigger"] not in self.loaded:
                continue
            self._reflected.add(key)
            self._cur = ("<root>", f"jdk-reflect:{rule['trigger']}")
            for c in rule.get("classes", []):
                if self.w.cp.find(c) is not None:
                    self.init_class(c)
                    self.reflect_all(c, "<init>")
            for pkg, pat in rule.get("packages", []):
                rp, rn = re.compile(pkg), re.compile(pat)
                if getattr(self, '_all_names', None) is None:
                    self._all_names = self.w.cp.all_names()
                for n in self._all_names:
                    m = rp.match(n)
                    if not m or not rn.fullmatch(n[m.end():]):
                        continue
                    ci = self.load(n)
                    if ci is None or ci.is_interface or ci.cf.access & 0x0400:
                        continue
                    if ci.cf.method("<init>", "()V") is None:
                        continue
                    self.init_class(n)
                    self.reflect_all(n, "<init>")
        for reg in JDK_STRING_REGISTRIES:
            if reg not in self.loaded or reg in self._reflected:
                continue
            self._reflected.add(reg)
            self._cur = ("<root>", f"jdk-registry:{reg}")
            ci = self.w.classes.get(reg)
            for m in ci.cf.methods:
                if m.code is None:
                    continue
                for ins in self.insns(ci, m):
                    if ins.op == "ldc" and isinstance(ins.arg, tuple) and ins.arg[0] == "string" and "." in ins.arg[1]:
                        cn = ins.arg[1].replace(".", "/")
                        try:
                            ok = self.w.get(cn) is not None
                        except LinkError:
                            ok = False
                        if ok and self.w.classes[cn].cf.method("<init>", "()V") is not None \
                                and not self.w.classes[cn].is_interface and not self.w.classes[cn].cf.access & 0x0400:
                            self.init_class(cn)
                            self.reflect_all(cn, "<init>")

    def annotation_type(self, name: str):
        ci = self.w.classes[name]
        self._cur = ("<root>", f"annotation:{name}")
        for m in ci.cf.methods:
            _, ret = parse_method_desc(m.desc)
            t = ret.lstrip("[")
            if t.startswith("L"):
                tc = self.load(t[1:-1])
                if tc is not None and tc.super is not None and tc.super.name == "java/lang/Enum":
                    self.init_class(tc.name)
        self.proxy_class([name])

    def proxy_class(self, interfaces: list[str]):
        name = proxy_class_name(interfaces)
        if name in self.proxies:
            return
        if any(self.load(i) is None for i in interfaces):
            return
        self.proxies.add(name)
        cf = make_proxy(self.w, name, interfaces)
        self.w.add_synthetic(cf)
        ci = self.load(name)
        self.instantiate(name)
        self.init_class(name)
        for m in cf.methods:
            self.mark(ci, m)
        # InvocationHandler.invokeDefault, default metotlara sanal olmayan biçimde ulaşır
        for i in interfaces:
            ic = self.w.classes[i]
            for c in [ic] + self.w.all_interfaces(ic):
                for m in c.cf.methods:
                    if m.code is not None and not m.is_static and not m.name.startswith("<"):
                        self.mark(c, m)

    def _fold_proxy(self, insns, idx):
        if self._cur[0] == "java/lang/reflect/Proxy":
            return
        start = -1
        for j in range(idx - 1, max(-1, idx - 65), -1):
            b = insns[j]
            if b.op == "anewarray" and b.arg.name == "java/lang/Class":
                start = j
                break
        if start < 0:
            self.report("proxy-dynamic", f"{self._cur[0]}.{self._cur[1]}: Proxy.{insns[idx].arg.name} with non-constant interfaces")
            return
        last_store = idx
        for k in range(idx - 1, start, -1):
            if insns[k].op == "aastore":
                last_store = k + 1
                break
        ifaces = []
        for b in insns[start:last_store]:
            if b.op == "ldc" and isinstance(b.arg, ClassConst) and not b.arg.name.startswith("["):
                ci = self.load(b.arg.name)
                if ci is not None and ci.is_interface:
                    ifaces.append(b.arg.name)
        if ifaces:
            saved = self._cur
            self.proxy_class(ifaces)
            self._cur = saved

    def _fold_jna_proxy(self, insns, idx):
        # JNA Native.load([ad, ]Interface.class[, seçenekler]): sabit Class<T> literalleri için derleme zamanı vekili sentezle.
        # Sondaki seçenek haritalarını / dönüştürücüleri ve yerel Class<T> depolarını aşacak kadar derin tara.
        jna_lib = self.w.get("com/sun/jna/Library")
        candidates: list[str] = []
        for j in range(idx - 1, max(-1, idx - 49), -1):
            b = insns[j]
            if b.op == "ldc" and isinstance(b.arg, ClassConst):
                cn = b.arg.name
                if cn.startswith("["):
                    continue
                ci = self.load(cn)
                if ci is not None and ci.is_interface:
                    candidates.append(cn)
        if not candidates:
            for b in insns[:idx]:
                if b.op == "ldc" and isinstance(b.arg, ClassConst) and not b.arg.name.startswith("["):
                    ci = self.load(b.arg.name)
                    if ci is not None and ci.is_interface:
                        candidates.append(b.arg.name)
        lib_ifaces = []
        if jna_lib is not None:
            for cn in candidates:
                ci = self.w.get(cn)
                if ci is not None and jna_lib in self.w.all_interfaces(ci):
                    lib_ifaces.append(cn)
        chosen = lib_ifaces or candidates
        saved = self._cur
        seen: set[str] = set()
        for cn in chosen:
            if cn not in seen:
                seen.add(cn)
                self.proxy_class([cn])
        self._cur = saved

    def reflect_all(self, name: str, only: Optional[str] = None):
        """Keep methods of `name` for reflective use (all of them, or those called `only`)."""
        ci = self.load(name)
        if ci is None:
            return
        self.reflected_seen.add(name)
        self._cur = ("<root>", f"reflection:{name}")
        for m in ci.cf.methods:
            if m.name == "<clinit>" or (only is not None and m.name != only):
                continue
            if m.is_abstract:
                # soyut bir metodun yansıtmalı çağrıları alıcının uygulamasına gider
                self.vcall(name, m.name, m.desc)
                continue
            if m.name == "<init>":
                if ci.is_interface or ci.cf.access & 0x0400:
                    continue
                self.instantiate(name)
            elif m.is_static:
                self.init_class(name)
            elif not (m.access & 0x0002):
                self.vcall(name, m.name, m.desc)
            self.mark(ci, m)

    REFLECT_LOOKUPS = {"getMethod": None, "getDeclaredMethod": None, "getConstructor": "<init>",
                       "getDeclaredConstructor": "<init>", "getConstructors": "<init>",
                       "getDeclaredConstructors": "<init>", "newInstance": "<init>",
                       # MethodHandles.Lookup
                       "findVirtual": None, "findStatic": None, "findSpecial": None,
                       "findConstructor": "<init>", "bind": None}

    def _fold_reflection(self, insns, idx):
        """X.class.getMethod("n", ...) / getConstructor(...) with constant receivers: keep the targets.

        Over-approximates: every class constant (directly, or through a local variable that
        holds one) and every string constant in the argument window is a candidate."""
        ins = insns[idx]
        what = self.REFLECT_LOOKUPS[ins.arg.name]
        local_cls: dict[int, set[str]] = {}
        # bu metotta Class.forName("sabit") ile adlandırılan sınıflar (alıcı bir yerel değişkende/alanda tutulur)
        forname: set[str] = set()
        for i in range(1, len(insns)):
            x = insns[i]
            if x.op == "invokestatic" and x.arg.owner == "java/lang/Class" and x.arg.name == "forName":
                for back in reversed(insns[max(0, i - 4):i]):
                    if back.op == "ldc" and isinstance(back.arg, tuple) and back.arg[0] == "string":
                        forname.add(back.arg[1].replace(".", "/"))
                        break
        for i in range(1, len(insns)):
            if insns[i].op == "astore" and insns[i - 1].op == "ldc" and isinstance(insns[i - 1].arg, ClassConst):
                local_cls.setdefault(insns[i].arg, set()).add(insns[i - 1].arg.name)
        classes: set[str] = set(n for n in forname if self.w.cp.find(n) is not None)
        names: set[str] = set()
        # Alıcı ilk itilir: ilk ad dizesinden / argümandan önceki sınıf sabitleri
        # dizisi alıcılardır; sonrakiler parametre türleridir.
        seen_arg = False
        # Değişken argüman biçimleri için kesin alıcı (`X.class.getMethod("n", T.class, ...)`): alıcı,
        # isteğe bağlı ad dizesinden ve parametre dizisinin `iconst n; anewarray` komutundan hemen önceki itmedir.
        # Bu, önceki ifadelerin dize/sınıf sabitlerini pencerenin dışında tutar.
        exact: Optional[int] = None
        lo = max(0, idx - 24)
        for j in range(idx - 1, lo, -1):
            if insns[j].op == "anewarray" and insns[j - 1].op in (
                    "iconst_0", "iconst_1", "iconst_2", "iconst_3", "iconst_4", "iconst_5", "bipush", "sipush"):
                k = j - 2
                if k >= 0 and insns[k].op == "ldc" and isinstance(insns[k].arg, tuple) and insns[k].arg[0] == "string":
                    k -= 1
                if k >= 0:
                    exact = k
                break
        if exact is not None:
            rx = insns[exact]
            if rx.op == "ldc" and isinstance(rx.arg, ClassConst):
                classes.add(rx.arg.name)
                seen_arg = True
            elif rx.op == "aload" and rx.arg in local_cls:
                classes |= local_cls[rx.arg]
                seen_arg = True
        for back in insns[lo:idx]:
            is_str = back.op == "ldc" and isinstance(back.arg, tuple) and back.arg[0] == "string"
            if is_str:
                names.add(back.arg[1])
            if is_str or back.op in ("anewarray", "iconst_0", "iconst_1", "iconst_2", "bipush"):
                if is_str or back.op == "anewarray":
                    seen_arg = True
            if seen_arg:
                continue
            if back.op == "ldc" and isinstance(back.arg, ClassConst):
                classes.add(back.arg.name)
            elif back.op == "aload" and back.arg in local_cls:
                classes |= local_cls[back.arg]
        if names and what != "<init>":
            # hiçbir sınıfın bildirmediği adlar: alıcı burada bir sabit değildir, bunları
            # programın her sınıf sabitine karşı çöz
            for n in names:
                if not any(n in self._method_names(self.w.classes[c]) for c in classes if c in self.w.classes):
                    self.dyn_names.add(n)
        for cls in classes:
            if cls.startswith("["):
                continue
            if what == "<init>":
                self.reflect_all(cls, "<init>")
                continue
            c = self.load(cls)
            while c is not None:
                for n in names:
                    self.reflect_all(c.name, n)
                c = c.super

    def chain(self, cls: str, key: str, limit: int = 40) -> list[str]:
        out = []
        k = (cls, key)
        while k in self.why and len(out) < limit:
            out.append(f"{k[0]}.{k[1]}")
            k = self.why[k]
            if k[0] == "<root>":
                break
        return out

    def scan(self, ci: ClassInfo, m: MethodInfo):
        self._cur = (ci.name, m.key)
        if (ci.name, m.name, m.desc) in self.skip_scan:
            return   # replaced by a Rust patch ("replace": true): the original's references are not part of the program
        for h in m.code.handlers:
            if h.catch_type:
                self.load(h.catch_type)
        insns = self.insns(ci, m)
        for idx, ins in enumerate(insns):
            # Class.forName("sabit") kapalı dünyada statik bir referanstır (GraalVM de aynı
            # katlamayı yapar): adlandırılan sınıfı yükle ve başlat.
            if ins.op == "invokestatic" and ins.arg.owner == "java/lang/Class" and ins.arg.name == "forName":
                for back in insns[max(0, idx - 4):idx]:
                    if back.op == "ldc" and isinstance(back.arg, tuple) and back.arg[0] == "string":
                        cname = back.arg[1].replace(".", "/")
                        try:
                            found = self.w.get(cname) is not None
                        except LinkError as e:
                            self.report("link-error", f"Class.forName(\"{cname}\"): {e}")
                            found = False
                        if found:
                            self.init_class(cname)
                        break
            if ins.op == "invokestatic" and ins.arg.owner == "java/lang/reflect/Proxy" \
                    and ins.arg.name in ("newProxyInstance", "getProxyClass"):
                self._fold_proxy(insns, idx)
            if ins.op == "invokestatic" and ins.arg.owner == "com/sun/jna/Native" \
                    and ins.arg.name in ("load", "loadLibrary"):
                self._fold_jna_proxy(insns, idx)
            if ins.op == "invokevirtual" and ins.arg.owner in ("java/lang/Class", "java/lang/invoke/MethodHandles$Lookup") \
                    and ins.arg.name in self.REFLECT_LOOKUPS:
                saved = self._cur
                self._fold_reflection(insns, idx)
                self._cur = saved
        # Class.forName(<hesaplanmış ad>): metodun, sınıf yolundaki bir sınıfı adlandıran her dize sabiti
        # bir adaydır (kapalı dünya aşırı yaklaşımı).
        # `class$(String)`, javac'ın 1.5 öncesi sınıf literali yardımcısıdır (JOML'un MemUtil'i hâlâ kullanır): kılık değiştirmiş bir forName
        if any(i.op == "invokestatic" and ((i.arg.owner == "java/lang/Class" and i.arg.name == "forName")
                                           or (i.arg.name.startswith("class$")
                                               and i.arg.desc == "(Ljava/lang/String;)Ljava/lang/Class;"))
               for i in insns):
            for i in insns:
                if i.op == "ldc" and isinstance(i.arg, tuple) and i.arg[0] == "string" \
                        and "." in i.arg[1] and " " not in i.arg[1]:
                    cn = i.arg[1].replace(".", "/")
                    try:
                        ok = self.w.get(cn) is not None
                    except LinkError:
                        ok = False
                    if ok:
                        self.init_class(cn)
                        self.instantiate(cn)
                        self.reflect_all(cn, "<init>")
        for ins in insns:
            op = ins.op
            a = ins.arg
            if op == "new":
                self.instantiate(a.name)
            elif op in ("anewarray", "checkcast", "instanceof", "multianewarray"):
                self.load(a.name)
            elif op == "ldc" and isinstance(a, ClassConst):
                if not a.name.startswith("["):
                    self.ldc_classes.add(a.name)
                # Bir Class nesnesi yansıtmalı olarak başlatılabilir (Class.forName,
                # Lookup.ensureInitialized, Unsafe.ensureClassInitialized): <clinit>'i tut.
                if not a.name.startswith("["):
                    self.init_class(a.name)
                else:
                    self.load(a.name)
            elif op == "ldc" and isinstance(a, tuple) and a[0] == "string":
                self.instantiate("java/lang/String")
            elif op in ("getstatic", "putstatic"):
                try:
                    dc, f = self.w.resolve_field(a.owner, a.name, a.desc)
                    self.load(dc.name)
                    self.init_class(dc.name)
                except LinkError as e:
                    self.report("link-error", str(e))
            elif op in ("getfield", "putfield"):
                try:
                    dc, f = self.w.resolve_field(a.owner, a.name, a.desc)
                    self.load(dc.name)
                except LinkError as e:
                    self.report("link-error", str(e))
            elif op == "invokestatic":
                r = self.resolve(a)
                if r:
                    self.init_class(r[0].name)
                    self.mark(*r)
            elif op == "invokespecial":
                r = self.resolve_special(ci, a)
                if r:
                    self.mark(*r)
            elif op == "invokevirtual" and a.owner == "java/lang/invoke/VarHandle" \
                    and a.name not in ("toString", "hashCode", "equals", "varType", "type", "getClass",
                                       # VarHandle kaplamasının sıradan metotları, erişim modları değil
                                       "accessModeType", "coordinateTypes", "withInvokeExactBehavior",
                                       "withInvokeBehavior", "hasInvokeExactBehavior"):
                self.load("java/lang/invoke/VarHandle")
                if a.desc.startswith("(Ljava/lang/foreign/MemorySegment;J") and "java/lang/foreign/MemorySegment" in self.w.classes:
                    self.load("java/lang/foreign/MemorySegment")
                    self.vcall("java/lang/foreign/MemorySegment", "address", "()J")
                continue
            elif op in ("invokevirtual", "invokeinterface"):
                r = self.resolve(a)
                if r is None:
                    continue
                dc, mm = r
                if mm.is_private or (a.owner.startswith("[") and mm.name != "clone"):
                    self.mark(dc, mm)
                if mm.is_private:
                    continue
                owner = a.owner if not a.owner.startswith("[") else "java/lang/Object"
                self.load(owner)
                if dc.name == "java/lang/Object" and self.w.classes[owner].is_interface:
                    owner = "java/lang/Object"
                self.vcall(owner, a.name, a.desc)

    def single_target(self, owner: str, name: str, desc: str):
        """Class hierarchy analysis over instantiated classes (closed world): the unique method
        an invokevirtual/invokeinterface on `owner` can dispatch to, or None if several."""
        key = (owner, name, desc)
        cache = self.__dict__.setdefault("_cha", {})
        if key in cache:
            return cache[key]
        oc = self.w.classes.get(owner)
        impl = None
        multiple = False
        if oc is not None and owner != "java/lang/Object":
            seen = set()
            stack = [oc]
            while stack and not multiple:
                c = stack.pop()
                if c.name in seen:
                    continue
                seen.add(c.name)
                if c.name in self.instantiated:
                    sel = self.w.select(c, name, desc)
                    t = (sel[0].name, sel[1].key) if sel else ("<none>", "")
                    if impl is None:
                        impl = t
                    elif impl != t:
                        multiple = True
                stack.extend(c.subclasses)
        res = None if (multiple or impl is None or impl[0] == "<none>") else impl
        cache[key] = res
        return res

    def small_targets(self, owner: str, name: str, desc: str, limit: int = 2):
        """For bounded polymorphic CHA dispatch: return tuple of (concrete_class, (impl_class, impl_key))
        if all instantiated subclasses of `owner` resolve within `limit` concrete classes, else None."""
        key = (owner, name, desc, limit)
        cache = self.__dict__.setdefault("_cha_small", {})
        if key in cache:
            return cache[key]
        oc = self.w.classes.get(owner)
        res = None
        if oc is not None and owner != "java/lang/Object":
            seen = set()
            stack = [oc]
            pairs = []
            ok = True
            while stack:
                c = stack.pop()
                if c.name in seen:
                    continue
                seen.add(c.name)
                if c.name in self.instantiated:
                    sel = self.w.select(c, name, desc)
                    if not sel:
                        ok = False
                        break
                    pairs.append((c.name, (sel[0].name, sel[1].key)))
                    if len(pairs) > limit:
                        ok = False
                        break
                stack.extend(c.subclasses)
            if ok and 1 < len(pairs) <= limit:
                pairs.sort()
                res = tuple(pairs)
        cache[key] = res
        return res

    def resolve_special(self, cur: ClassInfo, a: MemberRef):
        r = self.resolve(a)
        if r is None:
            return None
        dc, m = r
        # JVMS invokespecial: başvurulan sınıf bir üst sınıf olduğunda (ACC_SUPER anlambilimi),
        # super çağrısı araması, mevcut sınıfın doğrudan üst sınıfından başlar.
        if m.name != "<init>" and not dc.is_interface and not m.is_private:
            oc = self.w.classes.get(a.owner)
            if oc is not None and not oc.is_interface and oc is not cur and cur.super is not None \
                    and self.w.is_subclass(cur, oc):
                for x in cur.super.supers():
                    mm = x.cf.method(a.name, a.desc)
                    if mm and not mm.is_static:
                        return x, mm
                sel = self.w.select(cur.super, a.name, a.desc)
                if sel:
                    return sel
        return r
