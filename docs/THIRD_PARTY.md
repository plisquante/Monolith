# Third-party code in Monolith

Policy (docs/MASTER_PLAN_TR.md §7): a library is used when it is mature and better than writing our own. Every entry is pinned, vendored and listed here.

| Crate / library | Version | License | Where | Used by | Why |
|---|---|---|---|---|---|
| crc32fast | 1.4.2 | MIT OR Apache-2.0 | rt_deps/vendor/crc32fast | monolith/rt/natives_io.rs (java.util.zip.CRC32) | removes the zlib call for CRC32; pure Rust, SIMD where available |
| cfg-if | 1.0.x (transitive) | MIT OR Apache-2.0 | rt_deps/vendor/cfg-if | crc32fast | transitive |

Still system libraries (to be replaced or vendored, see docs/IMPL_SPEC.md): zlib (Inflater/Deflater, adler32), OpenAL, shaderc/SPIRV-Cross, libffi (JNA), X11/GLX, libc.

Compiling the dependency set happens automatically in monolith/rustgen.py (`rt_deps_flags`), offline, from rt_deps/. A failed compile stops generation; nothing falls back silently.
Update: edit rt_deps/Cargo.toml, refresh Cargo.lock, re-vendor into rt_deps/vendor, commit the diff, review the licenses.
