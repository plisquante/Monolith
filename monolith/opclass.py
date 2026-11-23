"""Opcode and intrinsic tables shared by the escape analysis and the code generators.

ARITH/CONV/IF1/IF2 classify bytecodes; INTRINSICS maps Java methods to runtime helpers. Escape analysis reads
only the keys and arities; the value strings are generator-specific and move with the generator.
"""
ARITH = {
    "iadd": ("I", "{0} +% {1}", False, 2), "isub": ("I", "{0} -% {1}", False, 2),
    "imul": ("I", "{0} *% {1}", False, 2), "idiv": ("I", "rt.idiv({0}, {1})", True, 2),
    "irem": ("I", "rt.irem({0}, {1})", True, 2), "ineg": ("I", "0 -% {0}", False, 1),
    "ishl": ("I", "rt.ishl({0}, {1})", False, 2), "ishr": ("I", "rt.ishr({0}, {1})", False, 2),
    "iushr": ("I", "rt.iushr({0}, {1})", False, 2), "iand": ("I", "{0} & {1}", False, 2),
    "ior": ("I", "{0} | {1}", False, 2), "ixor": ("I", "{0} ^ {1}", False, 2),
    "ladd": ("J", "{0} +% {1}", False, 2), "lsub": ("J", "{0} -% {1}", False, 2),
    "lmul": ("J", "{0} *% {1}", False, 2), "ldiv": ("J", "rt.ldiv({0}, {1})", True, 2),
    "lrem": ("J", "rt.lrem({0}, {1})", True, 2), "lneg": ("J", "0 -% {0}", False, 1),
    "lshl": ("J", "rt.lshl({0}, {1})", False, 2), "lshr": ("J", "rt.lshr({0}, {1})", False, 2),
    "lushr": ("J", "rt.lushr({0}, {1})", False, 2), "land": ("J", "{0} & {1}", False, 2),
    "lor": ("J", "{0} | {1}", False, 2), "lxor": ("J", "{0} ^ {1}", False, 2),
    "fadd": ("F", "{0} + {1}", False, 2), "fsub": ("F", "{0} - {1}", False, 2),
    "fmul": ("F", "{0} * {1}", False, 2), "fdiv": ("F", "{0} / {1}", False, 2),
    "frem": ("F", "rt.fmod32({0}, {1})", False, 2), "fneg": ("F", "-{0}", False, 1),
    "dadd": ("D", "{0} + {1}", False, 2), "dsub": ("D", "{0} - {1}", False, 2),
    "dmul": ("D", "{0} * {1}", False, 2), "ddiv": ("D", "{0} / {1}", False, 2),
    "drem": ("D", "rt.fmod64({0}, {1})", False, 2), "dneg": ("D", "-{0}", False, 1),
    "lcmp": ("I", "rt.lcmp({0}, {1})", False, 2),
    "fcmpl": ("I", "rt.fcmpl({0}, {1})", False, 2), "fcmpg": ("I", "rt.fcmpg({0}, {1})", False, 2),
    "dcmpl": ("I", "rt.dcmpl({0}, {1})", False, 2), "dcmpg": ("I", "rt.dcmpg({0}, {1})", False, 2),
}
CONV = {
    "i2l": ("J", "@as(i64, {0})"), "i2f": ("F", "@as(f32, @floatFromInt({0}))"),
    "i2d": ("D", "@as(f64, @floatFromInt({0}))"), "l2i": ("I", "@as(i32, @truncate({0}))"),
    "l2f": ("F", "@as(f32, @floatFromInt({0}))"), "l2d": ("D", "@as(f64, @floatFromInt({0}))"),
    "f2i": ("I", "rt.f2i({0})"), "f2l": ("J", "rt.f2l({0})"), "f2d": ("D", "@as(f64, {0})"),
    "d2i": ("I", "rt.d2i({0})"), "d2l": ("J", "rt.d2l({0})"), "d2f": ("F", "@as(f32, @floatCast({0}))"),
    "i2b": ("I", "rt.i2b({0})"), "i2c": ("I", "rt.i2c({0})"), "i2s": ("I", "rt.i2s({0})"),
}
IF1 = {"ifeq": "{0} == 0", "ifne": "{0} != 0", "iflt": "{0} < 0", "ifge": "{0} >= 0",
       "ifgt": "{0} > 0", "ifle": "{0} <= 0", "ifnull": "{0} == null", "ifnonnull": "{0} != null"}
IF2 = {"if_icmpeq": "{0} == {1}", "if_icmpne": "{0} != {1}", "if_icmplt": "{0} < {1}",
       "if_icmpge": "{0} >= {1}", "if_icmpgt": "{0} > {1}", "if_icmple": "{0} <= {1}",
       "if_acmpeq": "{0} == {1}", "if_acmpne": "{0} != {1}"}




def _intrinsics() -> dict:
    t: dict = {}
    for c in ("java/lang/Math", "java/lang/StrictMath"):
        for k, f, th in (("floor(D)D", "iFloor", 0), ("ceil(D)D", "iCeil", 0), ("sqrt(D)D", "iSqrt", 0),
                         ("abs(I)I", "iAbsI", 0), ("abs(J)J", "iAbsJ", 0), ("abs(F)F", "iAbsF", 0), ("abs(D)D", "iAbsD", 0),
                         ("min(II)I", "iMinI", 0), ("max(II)I", "iMaxI", 0), ("min(JJ)J", "iMinJ", 0), ("max(JJ)J", "iMaxJ", 0),
                         ("min(FF)F", "iMinF", 0), ("max(FF)F", "iMaxF", 0), ("min(DD)D", "iMinD", 0), ("max(DD)D", "iMaxD", 0),
                         ("copySign(DD)D", "iCopySignD", 0), ("copySign(FF)F", "iCopySignF", 0),
                         ("signum(D)D", "iSignumD", 0), ("signum(F)F", "iSignumF", 0),
                         ("fma(DDD)D", "iFmaD", 0), ("fma(FFF)F", "iFmaF", 0),
                         ("floorDiv(II)I", "iFloorDivI", 1), ("floorMod(II)I", "iFloorModI", 1),
                         ("floorDiv(JJ)J", "iFloorDivJ", 1), ("floorMod(JJ)J", "iFloorModJ", 1)):
            t[(c, k)] = (f, bool(th))
    for c in ("java/lang/Math", "java/lang/StrictMath"):
        for k, f in (("getExponent(D)I", "iExpD"), ("getExponent(F)I", "iExpF")):
            t[(c, k)] = (f, False)
    for k, f, th in (("compare(II)I", "iCmpI", 0), ("compareUnsigned(II)I", "iCmpUI", 0),
                     ("divideUnsigned(II)I", "iDivUI", 1), ("remainderUnsigned(II)I", "iRemUI", 1),
                     ("signum(I)I", "iSigI", 0), ("rotateLeft(II)I", "iRotlI", 0),
                     ("rotateRight(II)I", "iRotrI", 0), ("reverse(I)I", "iRevI", 0),
                     ("reverseBytes(I)I", "iRevBytesI", 0), ("hashCode(I)I", "iIdI", 0),
                     ("toUnsignedLong(I)J", "iToUnsignedLong", 0), ("bitCount(I)I", "iBitCountI", 0),
                     ("numberOfLeadingZeros(I)I", "iClzI", 0), ("numberOfTrailingZeros(I)I", "iCtzI", 0)):
        t[("java/lang/Integer", k)] = (f, bool(th))
    for k, f, th in (("compare(JJ)I", "iCmpJ", 0), ("compareUnsigned(JJ)I", "iCmpUJ", 0),
                     ("divideUnsigned(JJ)J", "iDivUJ", 1), ("remainderUnsigned(JJ)J", "iRemUJ", 1),
                     ("signum(J)I", "iSigJ", 0), ("rotateLeft(JI)J", "iRotlJ", 0),
                     ("rotateRight(JI)J", "iRotrJ", 0), ("reverse(J)J", "iRevJ", 0),
                     ("reverseBytes(J)J", "iRevBytesJ", 0), ("hashCode(J)I", "iHashJ", 0),
                     ("bitCount(J)I", "iBitCountJ", 0), ("numberOfLeadingZeros(J)I", "iClzJ", 0),
                     ("numberOfTrailingZeros(J)I", "iCtzJ", 0)):
        t[("java/lang/Long", k)] = (f, bool(th))
    t[("java/lang/Short", "reverseBytes(S)S")] = ("iRevBytesS", False)
    t[("java/lang/Character", "reverseBytes(C)C")] = ("iRevBytesC", False)
    for k, f in (("floatToRawIntBits(F)I", "iF2I"), ("floatToIntBits(F)I", "iFloatToIntBits"),
                 ("intBitsToFloat(I)F", "iI2F"), ("isNaN(F)Z", "iIsNanF"),
                 ("isInfinite(F)Z", "iIsInfF"), ("isFinite(F)Z", "iIsFinF")):
        t[("java/lang/Float", k)] = (f, False)
    for k, f in (("doubleToRawLongBits(D)J", "iD2J"), ("doubleToLongBits(D)J", "iDoubleToLongBits"),
                 ("longBitsToDouble(J)D", "iJ2D"), ("isNaN(D)Z", "iIsNanD"),
                 ("isInfinite(D)Z", "iIsInfD"), ("isFinite(D)Z", "iIsFinD")):
        t[("java/lang/Double", k)] = (f, False)
    return t


INTRINSICS = _intrinsics()
