"""Tiny bytecode assembler producing pre-decoded instruction lists for synthetic classes."""
from __future__ import annotations

from .bytecode import Insn
from .classfile import (ACC_PUBLIC, ACC_STATIC, ACC_SUPER, ACC_FINAL, ACC_SYNTHETIC, ClassConst,
                        ClassFile, Code, FieldInfo, MemberRef, MethodInfo,
                        CONSTANT_Fieldref, CONSTANT_Methodref, CONSTANT_InterfaceMethodref)
from .world import kind_of


def mref(owner: str, name: str, desc: str, interface: bool = False) -> MemberRef:
    return MemberRef(CONSTANT_InterfaceMethodref if interface else CONSTANT_Methodref, owner, name, desc)


def fref(owner: str, name: str, desc: str) -> MemberRef:
    return MemberRef(CONSTANT_Fieldref, owner, name, desc)


class Asm:
    def __init__(self):
        self.insns: list[Insn] = []
        self.labels: dict[str, int] = {}
        self.fix: list[tuple[Insn, list[str]]] = []
        self.pc = 0

    def op(self, op: str, arg=None, arg2=None) -> Insn:
        i = Insn(self.pc, op, arg, arg2)
        self.pc += 1
        self.insns.append(i)
        return i

    def load(self, desc: str, idx: int):
        self.op({"I": "iload", "J": "lload", "F": "fload", "D": "dload", "A": "aload"}[kind_of(desc)], idx)

    def ret(self, desc: str):
        if desc == "V":
            self.op("return")
        else:
            self.op({"I": "ireturn", "J": "lreturn", "F": "freturn", "D": "dreturn", "A": "areturn"}[kind_of(desc)])

    def label(self, name: str):
        self.labels[name] = self.pc

    def jump(self, op: str, label: str):
        i = self.op(op)
        self.fix.append((i, [label]))

    def iconst(self, v: int):
        self.op("ldc", ("int", v))

    def sconst(self, s: str):
        self.op("ldc", ("string", s))

    def finish(self) -> list[Insn]:
        for i, labs in self.fix:
            i.targets = tuple(self.labels[x] for x in labs)
        return self.insns


def make_method(name: str, desc: str, access: int, asm: Asm, max_locals: int) -> MethodInfo:
    m = MethodInfo(access, name, desc)
    m.code = Code(16, max_locals, b"", [])
    m.insns = asm.finish()  # type: ignore[attr-defined]
    return m


def make_class(name: str, super_name: str, interfaces: list[str], fields: list[FieldInfo],
               methods: list[MethodInfo], access: int = ACC_PUBLIC | ACC_SUPER | ACC_FINAL | ACC_SYNTHETIC
               ) -> ClassFile:
    cf = ClassFile(0, 69, access, name, super_name, interfaces, fields, methods, [None])
    cf.source_file = "<synthetic>"
    return cf


__all__ = ["Asm", "mref", "fref", "make_method", "make_class", "ClassConst",
           "ACC_PUBLIC", "ACC_STATIC", "ACC_FINAL", "ACC_SYNTHETIC"]
