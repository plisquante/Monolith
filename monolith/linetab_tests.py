"""C9 yan tablosu (monolith/linetab.py) için küçük testler.

Çalıştırma: python3 -m monolith.linetab_tests
"""
import sys

from monolith import linetab

SRC = '''#[inline] fn r_A__f__1(this: rt::Ref) -> rt::Res<i32> {
    static FI: rt::FrameInfo = rt::FrameInfo { class: "p/A", method: "f", desc: "()I", file: "A.java" };
    let _frame = rt::enter_frame_s(&FI)?;
    rt::set_line_s(10);
    s0a = foo()?;
    rt::set_line_s(11);
    return Ok(1);
}
#[no_mangle] pub extern "C" fn m_A__f__1(this: rt::Ref) -> rt::CRes<i32> {
    rt::cres(r_A__f__1(this))
}
fn r_B__g__2() -> rt::Res<()> {
    static FI: rt::FrameInfo = rt::FrameInfo { class: "p/B$\\"q\\"", method: "g", desc: "()V", file: "B.java" };
    let _frame = rt::enter_frame_s(&FI)?;
    bar()?;
    rt::set_line_s(7);
    baz()?;
    return Ok(());
}'''

fails = 0


def check(name, got, want):
    global fails
    ok = got == want
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else f": {got!r} != {want!r}"))
    fails += not ok


new, tab = linetab.build(SRC, strip=True)
check("satir sayisi korunur", len(new.split("\n")), len(SRC.split("\n")))
check("set_line_s kalmaz", new.count("set_line_s("), 0)
check("enter_frame_s yerine stack_check", new.count("rt::stack_check()?;"), 2)
check("A.f satir 10", linetab.lookup(tab, 5), ("p/A", "f", "()I", "A.java", 10))
check("A.f satir 11", linetab.lookup(tab, 7), ("p/A", "f", "()I", "A.java", 11))
check("sarmalayici Java degil", linetab.lookup(tab, 10), None)
check("B.g satir isareti yok (-1)", linetab.lookup(tab, 15), ('p/B$"q"', "g", "()V", "B.java", -1))
check("B.g satir 7", linetab.lookup(tab, 17), ('p/B$"q"', "g", "()V", "B.java", 7))
same, _ = linetab.build(SRC, strip=False)
check("strip=False metni degistirmez", same, SRC)
print("ALL PASSED" if not fails else f"{fails} FAILED")
sys.exit(1 if fails else 0)
