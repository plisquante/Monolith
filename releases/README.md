# Minecraft 26.3 Android (`arm64-v8a`) Build

- `Monolith.apk` (44.67 MiB) — signed TouchCore launcher (`apksigner` v2/v3 verified)
- `libminecraft_mono.so.xz.00` + `libminecraft_mono.so.xz.01` — `libminecraft_mono.so` (693 MiB uncompressed) split into <= 80 MiB `xz` chunks to satisfy GitHub's 100 MiB per-file limit.

Reassemble and decompress:
```sh
cat libminecraft_mono.so.xz.00 libminecraft_mono.so.xz.01 | xz -d > libminecraft_mono.so
```
Expected `libminecraft_mono.so` SHA-256: `5597ee1baef6fd37575c0a625dd745c72486951718b449a1aba007c7c17df1b7`
