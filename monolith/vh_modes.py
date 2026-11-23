"""VarHandle access modes shared by the code generators: name -> (family, number of value operands, volatile)."""
VH_MODES = {}
for _n in ("get", "getOpaque"):
    VH_MODES[_n] = ("get", 0, False)
for _n in ("getVolatile", "getAcquire"):
    VH_MODES[_n] = ("get", 0, True)
for _n in ("set", "setOpaque"):
    VH_MODES[_n] = ("set", 1, False)
for _n in ("setVolatile", "setRelease"):
    VH_MODES[_n] = ("set", 1, True)
for _n in ("compareAndSet", "weakCompareAndSet", "weakCompareAndSetPlain", "weakCompareAndSetAcquire",
           "weakCompareAndSetRelease"):
    VH_MODES[_n] = ("cas", 2, True)
for _n in ("compareAndExchange", "compareAndExchangeAcquire", "compareAndExchangeRelease"):
    VH_MODES[_n] = ("cax", 2, True)
for _n in ("getAndSet", "getAndSetAcquire", "getAndSetRelease"):
    VH_MODES[_n] = ("xchg", 1, True)
for _p, _f in (("getAndAdd", "add"), ("getAndBitwiseOr", "bor"), ("getAndBitwiseAnd", "band"),
               ("getAndBitwiseXor", "bxor")):
    for _sfx in ("", "Acquire", "Release"):
        VH_MODES[_p + _sfx] = (_f, 1, True)
