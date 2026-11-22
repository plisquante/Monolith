# Java residue

How much of the running program is still translated Java bytecode. Measured by `python3 -m monolith.residue`
(Minecraft 26.3, `--no-lwjgl`, desktop build); baseline in `residue-baseline.json`.

| Origin | Methods with a Java body | Bytecode |
|---|---:|---:|
| Minecraft (the client archive, 9,824 classes) | 88,385 | 3,812 KiB (44%) |
| JDK (java.base 2,175 KiB, java.xml 419, java.net.http 118, ...) | 139,023 | 2,914 KiB (33%) |
| Libraries (icu4j 479, fastutil 405, log4j-core 208, guava 118, oshi 98, netty ~125, joml 64, ...) | 49,760 | 1,921 KiB (22%) |
| Our Java overlay (102 classes) | 1,339 | 51 KiB (0.6%) |
| **Total** | **278,507** (of 291,066 compiled) | **8,699 KiB** |

Largest game packages: world/level 732 KiB, world/entity 470, util/datafix 333, client/gui 274, client/renderer 247,
world/item 179.

The running program contains no JVM and no bytecode: every one of these methods is already Rust, generated from the
bytecode. "Residue" means Rust that was generated rather than written. A method leaves the residue when a Rust patch
replaces it (`patches/rust/manifest.json`) and its original is no longer called.
