#!/usr/bin/env python3
"""Link-error gate: every `link-error` line of a verbose transpile must belong to a known, explained group.

    MONO_VERBOSE=1 python3 -m monolith.rustgen ... 2> transpile.err
    python3 tools/check_link_errors.py transpile.err

A link error is a call site, field access or catch clause that names a class or member that is not on the class path. The
generated code throws NoSuchMethodError, NoSuchFieldError or NoClassDefFoundError there, at run time and only if the site is
reached, exactly where the JVM would. Groups that are expected (see docs/LINK_ERRORS_TR.md):

  log4j-core        optional appenders and layouts (JMS, SMTP, ZeroMQ, Kafka, Disruptor, Jackson, ...) whose libraries are not shipped
  netty-log4j       netty's optional log4j 1.x logger adapter
  commons-compress  optional codecs (zstd, xz) of commons-compress
  lambdaform        the JDK's LambdaForm / ClassSpecializer machinery, replaced by the monolith MethodHandle overlay
  macos-objc        macOS-only code (MacosUtil, NarratorMac) that uses the ca.weblite.objc bridge, a macOS-only library

Exit status 1 and the offending lines if anything else appears: a new link error is a regression until it is explained.
"""
import collections
import sys

GROUPS = (
    ("macos-objc", lambda owner, msg: "ca/weblite/objc" in msg),
    ("log4j-core", lambda owner, msg: owner.startswith("org/apache/logging/log4j/")),
    ("netty-log4j", lambda owner, msg: owner.startswith("io/netty/util/internal/logging/") or owner.startswith("io/netty/util/internal/")),
    ("commons-compress", lambda owner, msg: owner.startswith("org/apache/commons/compress/")),
    ("lambdaform", lambda owner, msg: owner.startswith("java/lang/invoke/") or owner.startswith("jdk/internal/foreign/")),
)
PREFIX = "  link-error: "


def classify(line: str) -> str | None:
    body = line[len(PREFIX):].rstrip("\n")
    owner = body.split(":", 1)[0].split(".")[0]
    for name, test in GROUPS:
        if test(owner, body):
            return name
    return None


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    counts: collections.Counter = collections.Counter()
    unexpected = []
    with open(sys.argv[1], errors="replace") as fh:
        for line in fh:
            if not line.startswith(PREFIX):
                continue
            g = classify(line)
            if g is None:
                unexpected.append(line.rstrip("\n"))
            else:
                counts[g] += 1
    total = sum(counts.values()) + len(unexpected)
    if total == 0:
        print("no link-error lines found: was the transpile run with MONO_VERBOSE=1?")
        return 2
    for g, n in counts.most_common():
        print(f"{n:5d}  {g}")
    print(f"{total:5d}  total, {len(unexpected)} unexplained")
    for u in unexpected[:40]:
        print("UNEXPLAINED:", u[:300])
    return 1 if unexpected else 0


if __name__ == "__main__":
    sys.exit(main())
