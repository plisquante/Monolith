# Allocation profile

- run time: 241 s; allocated 15,998.1 MiB in 360,663,774 objects (15998.1 MiB, 66.4 MiB/s, 3.9 GiB/min)
- escape analysis marked 0.0 MiB (0.0%) as non-escaping (arena candidates); the scope arena is switched off, so today they come from the collected heap
- 18 collections, total pause 39,329 ms (longest 6,277 ms); live after the last: 1,347.2 MiB, heap 2,899.3 MiB

## Where the allocated bytes go, by lifetime class

| class | share of allocated bytes | MiB |
|---|---:|---:|
| TEMP | 76.0% | 12,156.4 |
| MIXED | 20.3% | 3,242.2 |
| LONG | 3.7% | 598.9 |
| NEW | 0.0% | 0.7 |

TEMP is what must stop reaching a collector (stack, arena, no allocation); LONG is what must be owned (freed by `Drop` when its chunk/world goes); MIXED needs the allocating method looked at.

## Top 30 classes by allocated bytes

| class | allocated MiB | objects | avg B | survive>=1 GC | class | live MiB | arena MiB |
|---|---:|---:|---:|---:|---|---:|---:|
| [C | 1,248.6 | 7,669,193 | 170 | 1% | TEMP | 13.0 | 0.0 |
| [Ljava/lang/Object; | 1,042.8 | 14,197,323 | 77 | 12% | MIXED | 109.1 | 0.0 |
| java/util/Optional | 771.8 | 25,291,356 | 32 | 6% | TEMP | 42.7 | 0.0 |
| [J | 640.8 | 5,767,252 | 116 | 16% | MIXED | 103.2 | 0.0 |
| net/minecraft/core/BlockPos | 593.8 | 19,456,493 | 32 | 3% | TEMP | 9.1 | 0.0 |
| net/minecraft/world/phys/Vec3 | 545.4 | 11,913,895 | 48 | 0% | TEMP | 0.1 | 0.0 |
| [B | 528.1 | 3,103,865 | 178 | 10% | TEMP | 53.0 | 0.0 |
| java/util/ArrayList$Itr | 465.0 | 10,158,375 | 48 | 0% | TEMP | 0.0 | 0.0 |
| com/mojang/serialization/DataResult$Success | 410.8 | 13,460,284 | 32 | 0% | TEMP | 0.0 | 0.0 |
| net/minecraft/world/phys/AABB | 379.6 | 6,219,147 | 64 | 1% | TEMP | 1.6 | 0.0 |
| com/mojang/datafixers/RewriteResult | 283.1 | 9,276,411 | 32 | 15% | MIXED | 42.2 | 0.0 |
| com/mojang/datafixers/View | 283.1 | 9,276,411 | 32 | 15% | MIXED | 42.2 | 0.0 |
| it/unimi/dsi/fastutil/objects/ObjectArrayList$1 | 279.2 | 6,099,413 | 48 | 0% | TEMP | 0.0 | 0.0 |
| [I | 273.1 | 2,161,776 | 132 | 2% | TEMP | 5.8 | 0.0 |
| net/minecraft/world/phys/shapes/CubePointRange | 223.1 | 7,310,738 | 32 | 0% | TEMP | 0.0 | 0.0 |
| java/util/ImmutableCollections$ListItr | 220.3 | 4,811,825 | 48 | 0% | TEMP | 0.0 | 0.0 |
| com/mojang/datafixers/types/Func | 212.2 | 4,636,557 | 48 | 30% | MIXED | 63.0 | 0.0 |
| com/mojang/datafixers/functions/Id | 209.7 | 4,580,604 | 48 | 30% | MIXED | 62.4 | 0.0 |
| java/lang/String | 202.3 | 6,627,681 | 32 | 4% | TEMP | 5.3 | 0.0 |
| [D | 173.7 | 3,281,161 | 55 | 2% | TEMP | 2.5 | 0.0 |
| java/util/RegularEnumSet$EnumSetIterator | 156.2 | 3,412,064 | 48 | 0% | TEMP | 0.0 | 0.0 |
| net/minecraft/world/level/ChunkPos | 153.3 | 5,023,165 | 32 | 0% | TEMP | 0.3 | 0.0 |
| java/util/BitSet | 145.4 | 4,764,328 | 32 | 27% | MIXED | 39.0 | 0.0 |
| java/util/concurrent/ConcurrentHashMap$Node | 140.7 | 3,074,145 | 48 | 54% | MIXED | 63.4 | 0.0 |
| net/minecraft/world/phys/shapes/ArrayVoxelShape | 129.6 | 2,124,151 | 64 | 1% | TEMP | 0.8 | 0.0 |
| com/mojang/datafixers/util/Pair | 124.0 | 4,061,813 | 32 | 0% | TEMP | 0.3 | 0.0 |
| net/minecraft/world/phys/shapes/OffsetDoubleList | 120.9 | 3,962,190 | 32 | 0% | TEMP | 0.0 | 0.0 |
| java/util/HashMap$Node | 110.3 | 2,408,668 | 48 | 7% | TEMP | 7.3 | 0.0 |
| net/minecraft/world/phys/shapes/VoxelShape$$Lambda$5 | 106.6 | 2,328,526 | 48 | 0% | TEMP | 0.0 | 0.0 |
| java/util/ArrayList | 89.0 | 1,944,057 | 48 | 2% | TEMP | 1.6 | 0.0 |

## Long-lived data: classes with the most live bytes after the last collection

| class | live MiB | objects | allocated MiB | age 0/1/2/3+ (MiB) |
|---|---:|---:|---:|---|
| [Ljava/lang/Object; | 109.1 | 1,547,093 | 1,042.8 | 3.1/11.8/11.2/82.9 |
| [J | 103.2 | 1,339,570 | 640.8 | 6.1/22.8/33.8/40.6 |
| java/util/concurrent/ConcurrentHashMap$Node | 63.4 | 1,384,033 | 140.7 | 0.0/0.0/0.0/63.3 |
| com/mojang/datafixers/TypeRewriteRule$Everywhere | 63.1 | 1,377,634 | 63.1 | 0.0/0.0/0.0/63.1 |
| com/mojang/datafixers/types/Type$RewriteCacheKey | 63.1 | 1,377,397 | 63.1 | 0.0/0.0/0.0/63.1 |
| com/mojang/datafixers/TypeRewriteRule$OrElse | 63.0 | 1,377,111 | 63.0 | 0.0/0.0/0.0/63.0 |
| com/mojang/datafixers/types/Func | 63.0 | 1,375,862 | 212.2 | 0.0/0.0/0.0/63.0 |
| com/mojang/datafixers/functions/Id | 62.4 | 1,362,405 | 209.7 | 0.0/0.0/0.0/62.4 |
| [B | 53.0 | 46,319 | 528.1 | 2.4/12.7/23.1/14.8 |
| java/util/Optional | 42.7 | 1,399,852 | 771.8 | 0.0/0.0/0.0/42.7 |
| com/google/common/collect/RegularImmutableList | 42.6 | 1,395,551 | 48.6 | 0.0/0.0/0.0/42.6 |
| com/mojang/datafixers/RewriteResult | 42.2 | 1,383,892 | 283.1 | 0.0/0.0/0.0/42.2 |
| com/mojang/datafixers/View | 42.2 | 1,383,892 | 283.1 | 0.0/0.0/0.0/42.2 |
| com/mojang/datafixers/TypeRewriteRule$Seq | 42.0 | 1,377,485 | 42.0 | 0.0/0.0/0.0/42.0 |
| com/mojang/datafixers/TypeRewriteRule$All | 42.0 | 1,377,111 | 42.0 | 0.0/0.0/0.0/42.0 |
| java/util/BitSet | 39.0 | 1,276,846 | 145.4 | 0.0/0.1/0.1/38.8 |
| com/mojang/datafixers/types/Type$$Lambda$8 | 21.0 | 1,377,111 | 21.0 | 0.0/0.0/0.0/21.0 |
| com/mojang/datafixers/types/templates/Sum$SumType | 15.0 | 245,659 | 78.2 | 0.0/0.0/0.0/15.0 |
| net/minecraft/client/resources/model/geometry/BakedQuad | 13.7 | 149,288 | 13.7 | 0.0/0.0/0.0/13.7 |
| [C | 13.0 | 175,986 | 1,248.6 | 0.1/0.3/0.4/12.1 |
| net/minecraft/util/ThreadingDetector | 12.9 | 210,563 | 15.3 | 0.6/5.4/6.9/0.0 |
| com/mojang/datafixers/types/templates/Product$ProductType | 12.3 | 201,486 | 65.6 | 0.0/0.0/0.0/12.3 |
| net/minecraft/world/ticks/ScheduledTick | 10.8 | 176,343 | 14.5 | 0.2/10.4/0.1/0.0 |
| net/minecraft/client/model/geom/ModelPart$Vertex | 10.3 | 223,992 | 13.8 | 0.0/0.0/0.0/10.3 |
| java/util/concurrent/locks/ReentrantLock$NonfairSync | 9.6 | 210,678 | 11.5 | 0.4/4.0/5.2/0.0 |
| java/util/concurrent/Semaphore$NonfairSync | 9.6 | 210,563 | 11.5 | 0.4/4.0/5.2/0.0 |
| net/minecraft/world/level/chunk/PalettedContainer | 9.6 | 210,563 | 11.5 | 0.4/4.0/5.2/0.0 |
| net/minecraft/world/level/chunk/PalettedContainer$Data | 9.6 | 210,563 | 12.3 | 0.5/4.0/5.2/0.0 |
| net/minecraft/core/BlockPos | 9.1 | 297,929 | 593.8 | 0.3/5.8/3.0/0.0 |
| com/mojang/datafixers/types/templates/Tag$TagType | 8.7 | 190,926 | 11.4 | 0.0/0.0/0.0/8.7 |

## Methods that allocate the most (sampled, one sample per 65,536 B)

| allocating method | est. MiB | share | TEMP part | top classes | called from |
|---|---:|---:|---:|---|---|
| java.util.Arrays.copyOf | 657.1 | 4.3% | 65% | [C 63%, Object; 35%, [B 1% | AbstractStringBuilder.ensureCapacityInternal, ArrayList.toArray |
| it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap.clone | 463.2 | 3.0% | 0% | Object; 50%, [J 50% | SkyLightSectionStorage$SkyDataLayerStorageMap.copy, BlockLightSectionStorage$BlockDataLayerStorageMap.copy |
| net.minecraft.world.phys.shapes.VoxelShape.move | 324.0 | 2.1% | 100% | OffsetDoubleList 39%, CubePointRange 37%, ArrayVoxelShape 24% | BlockCollisions.computeNext |
| java.util.Optional.map | 298.6 | 2.0% | 86% | Optional 80%, View 14%, DataResult$Success 6% | TypeRewriteRule$Seq.cap1, View.rewrite |
| net.minecraft.server.level.ServerLevel.tickChunk | 298.2 | 2.0% | 100% | BlockPos 100% | ServerChunkCache.lambda_tickChunks_0 |
| com.mojang.datafixers.types.Type.everywhere | 285.1 | 1.9% | 15% | TypeRewriteRule$OrElse 22%, Type$RewriteCacheKey 21%, TypeRewriteRule$Everywhere 21% | TypeRewriteRule$Everywhere.rewrite, Check$CheckType.everywhere |
| it.unimi.dsi.fastutil.objects.AbstractObjectList.listIterator | 272.2 | 1.8% | 100% | ObjectArrayList$1 100% | AbstractObjectList.iterator |
| java.io.BufferedReader._init_ | 251.0 | 1.6% | 100% | [C 100% | Resource.openAsReader, ResourceMetadata.fromJsonStream |
| java.lang.String._init_ | 248.4 | 1.6% | 100% | [C 100% | StringBuilder.toString, String.substring |
| com.mojang.datafixers.functions.Functions.id | 212.9 | 1.4% | 0% | Func 100% | RewriteResult.nop |
| com.mojang.serialization.DataResult$Success.result | 210.1 | 1.4% | 100% | Optional 100% | DataResult$Instance.ap2, DataResult$Instance.ap3 |
| com.mojang.datafixers.RewriteResult.nop | 204.1 | 1.3% | 0% | Id 100% | TypeRewriteRule$Seq.rewrite, TypeRewriteRule$Nop.rewrite |
| com.mojang.datafixers.types.Type.rewrite | 193.5 | 1.3% | 67% | ConcurrentHashMap$Node 33%, ConcurrentHashMap$ReservationNode 25%, Type$$Lambda$1 22% | Type.everywhere |
| com.mojang.datafixers.TypeRewriteRule$Seq.rewrite | 165.0 | 1.1% | 25% | BitSet 26%, Optional 25%, View 25% | Type.rewrite, TypeRewriteRule$Seq.cap1 |
| com.mojang.datafixers.TypeRewriteRule$Nop.rewrite | 156.2 | 1.0% | 24% | View 26%, RewriteResult 25%, Optional 24% | TypeRewriteRule$OrElse.rewrite |
| net.minecraft.server.level.ServerChunkCache.getChunk | 154.6 | 1.0% | 100% | ServerChunkCache$$Lambda$5 40%, ChunkPos 39%, Ticket 20% | Level.getChunk |
| net.minecraft.world.phys.shapes.IndirectMerger._init_ | 145.9 | 1.0% | 100% | [I 58%, [D 42% | Shapes.createIndexMerger |
| java.nio.HeapByteBuffer._init_ | 145.4 | 1.0% | 100% | [B 100% | ByteBuffer.allocate |
| java.util.BitSet._init_ | 143.0 | 0.9% | 0% | [J 100% | RewriteResult.nop, BitSetDiscreteVoxelShape._init_ |
| net.minecraft.world.level.entity.EntitySectionStorage.forEachAccessibleNonEmptySection | 142.5 | 0.9% | 100% | LongAVLTreeSet$Subset$SubsetIterator 58%, LongAVLTreeSet$Subset 42% | EntitySectionStorage.getEntities |
| net.minecraft.world.phys.shapes.Shapes.create | 132.8 | 0.9% | 100% | [D 71%, ArrayVoxelShape 29%, BitSetDiscreteVoxelShape 0% | BlockCollisions._init_, Shapes.box |
| net.minecraft.world.level.Level.getEntities | 127.1 | 0.8% | 100% | Int2ObjectOpenHashMap$ValueIterator 37%, ArrayList 32%, Level$$Lambda$1 25% | EntityGetter.getEntityCollisions, Level.getPushableEntities |
| java.io.DataInputStream.readUTF | 125.9 | 0.8% | 100% | [B 54%, String 46% | CompoundTag$1.readString, StringTag$1.readAccounted |
| net.minecraft.world.entity.EntityFluidInteraction.getFluidHeight | 115.0 | 0.8% | 100% | ArrayList$Itr 100% | EntityFluidInteraction.isInFluid, Entity.getFluidHeight |
| net.minecraft.world.phys.shapes.VoxelShape.findIndex | 104.9 | 0.7% | 100% | VoxelShape$$Lambda$5 100% | VoxelShape.collideX, VoxelShape.calculateFace |
| net.minecraft.world.level.chunk.PalettedContainer.reencodeContents | 98.9 | 0.6% | 100% | [I 100% | PalettedContainer.pack |
| java.util.HashMap.resize | 98.4 | 0.6% | 0% | ? 100% | HashMap.putVal, HashMap.computeIfAbsent |
| net.minecraft.world.phys.Vec3.subtract | 98.2 | 0.6% | 100% | Vec3 100% | Entity.baseTick, BlockGetter.forEachBlockIntersectedBetween |
| java.util.HashMap.putVal | 96.1 | 0.6% | 99% | HashMap$Node 99%, LinkedHashMap$Entry 1% | HashMap.put, HashMap.putIfAbsent |
| java.lang.String.charsetCodeByName | 95.9 | 0.6% | 100% | [C 100% | String.charsetCode |

## First patch candidates: methods producing the most TEMP garbage

A Rust patch for these allocates nothing (stack values, reused buffers, iterators without boxing) or a scope arena.

| method | TEMP MiB (est.) | classes |
|---|---:|---|
| java.util.Arrays.copyOf | 423.9 | [C, [B |
| net.minecraft.world.phys.shapes.VoxelShape.move | 324.0 | OffsetDoubleList, CubePointRange, ArrayVoxelShape |
| net.minecraft.server.level.ServerLevel.tickChunk | 298.2 | BlockPos |
| it.unimi.dsi.fastutil.objects.AbstractObjectList.listIterator | 272.2 | ObjectArrayList$1 |
| java.util.Optional.map | 257.2 | Optional, DataResult$Success |
| java.io.BufferedReader._init_ | 251.0 | [C |
| java.lang.String._init_ | 248.4 | [C |
| com.mojang.serialization.DataResult$Success.result | 210.1 | Optional |
| net.minecraft.server.level.ServerChunkCache.getChunk | 154.6 | ServerChunkCache$$Lambda$5, ChunkPos, Ticket |
| net.minecraft.world.phys.shapes.IndirectMerger._init_ | 145.9 | [I, [D |
| java.nio.HeapByteBuffer._init_ | 145.4 | [B |
| net.minecraft.world.level.entity.EntitySectionStorage.forEachAccessibleNonEmptySection | 142.5 | LongAVLTreeSet$Subset$SubsetIterator, LongAVLTreeSet$Subset |
| net.minecraft.world.phys.shapes.Shapes.create | 132.8 | [D, ArrayVoxelShape, BitSetDiscreteVoxelShape |
| com.mojang.datafixers.types.Type.rewrite | 130.1 | ConcurrentHashMap$ReservationNode, Type$$Lambda$1, Type$$Lambda$2 |
| net.minecraft.world.level.Level.getEntities | 127.1 | Int2ObjectOpenHashMap$ValueIterator, ArrayList, Level$$Lambda$1 |
| java.io.DataInputStream.readUTF | 125.9 | [B, String |
| net.minecraft.world.entity.EntityFluidInteraction.getFluidHeight | 115.0 | ArrayList$Itr |
| net.minecraft.world.phys.shapes.VoxelShape.findIndex | 104.9 | VoxelShape$$Lambda$5 |
| net.minecraft.world.level.chunk.PalettedContainer.reencodeContents | 98.9 | [I |
| net.minecraft.world.phys.Vec3.subtract | 98.2 | Vec3 |
| java.lang.String.charsetCodeByName | 95.9 | [C |
| java.util.HashMap.putVal | 95.1 | HashMap$Node |
| net.minecraft.world.attribute.EnvironmentAttributeSystem$ValueSampler.computeValuePositional | 93.2 | ImmutableCollections$ListItr |
| net.minecraft.world.phys.Vec3.atCenterOf | 92.7 | Vec3 |
| java.lang.String.decode | 88.2 | [C |
| net.minecraft.world.level.NaturalSpawner.spawnForChunk | 86.2 | NaturalSpawner$$Lambda$4, NaturalSpawner$$Lambda$5, ArrayList$Itr |
| com.mojang.datafixers.TypeRewriteRule$Seq.cap1 | 84.5 | TypeRewriteRule$Seq$$Lambda$1 |
| net.minecraft.world.phys.shapes.Shapes.joinIsNotEmpty | 81.5 | IndirectMerger, CubePointRange, Shapes$$Lambda$3 |
| net.minecraft.world.phys.AABB.deflate | 77.1 | AABB |
| net.minecraft.util.ClassInstanceMultiMap.iterator | 71.9 | ArrayList$Itr, Iterators$1 |

## Allocation by thread

| thread | est. MiB | share |
|---|---:|---:|
| ? | 14,251.8 | 87.7% |
| main | 2,002.3 | 12.3% |

## Collections

| # | time s | live MiB | heap MiB | allocated since the previous MiB | pause ms |
|---:|---:|---:|---:|---:|---:|
| 1 | 0.2 | 7.9 | 27.1 | 32.1 | 21.0 |
| 2 | 0.2 | 12.2 | 28.2 | 16.0 | 24.2 |
| 3 | 0.4 | 18.9 | 38.2 | 24.6 | 36.7 |
| 4 | 0.5 | 36.8 | 57.6 | 37.8 | 58.2 |
| 5 | 0.8 | 54.9 | 111.8 | 73.6 | 127.7 |
| 6 | 1.2 | 77.9 | 167.1 | 109.8 | 220.2 |
| 7 | 1.9 | 105.1 | 237.8 | 155.9 | 345.4 |
| 8 | 2.8 | 140.2 | 318.4 | 210.2 | 492.4 |
| 9 | 4.1 | 190.2 | 411.8 | 280.6 | 706.0 |
| 10 | 6.1 | 254.2 | 549.5 | 380.4 | 1108.5 |
| 11 | 9.0 | 339.6 | 722.3 | 508.5 | 1491.8 |
| 12 | 13.3 | 510.1 | 1,046.5 | 679.2 | 2368.6 |
| 13 | 20.9 | 774.1 | 1,566.7 | 1,020.2 | 3774.9 |
| 14 | 31.1 | 997.2 | 1,761.6 | 1,548.2 | 4876.0 |
| 15 | 77.9 | 1,070.0 | 1,895.9 | 1,994.5 | 5491.9 |
| 16 | 99.9 | 1,223.5 | 2,382.1 | 2,140.2 | 5755.7 |
| 17 | 122.9 | 1,336.9 | 2,855.2 | 2,447.2 | 6277.0 |
| 18 | 184.4 | 1,347.2 | 2,899.3 | 2,673.8 | 6152.9 |

## Reading it for the GC-free plan

- Without a collector the program would allocate 3.9 GiB per minute; a 16 GiB machine lasts 4.1 minutes.
- If every TEMP allocation stops reaching the heap, about 24% of today's volume remains (0.93 GiB/min): that part must be owned and freed (chunks, worlds), not leaked.
- Data that really stays (LONG plus the kept part of MIXED) is about 11% of the volume.

## Inliner + iteration-local escape analysis + frame allocation + tickChunk patch (prof5)

201 s window after 160 s, same scripted play (`tools/headless_play.sh`, 240 s, `MONO_ARENA_CHECK=abort`): no crash, no arena violation.

| | prof4 (iterator recycling) | prof5 (this version) |
|---|---:|---:|
| allocated (heap and recycled, frame objects not counted) | 7,309.8 MiB, 36.5 MiB/s | 6,369.9 MiB, 31.7 MiB/s |
| objects | 180.8 M | 151.3 M |
| served without the collected heap (recycled iterators, arenas) | 1,437.6 MiB (19.7 %) | 53.1 MiB (0.8 %) |
| BlockPos | 745.4 MiB | 284.7 MiB (tickChunk) |
| Vec3 / AABB | 596.3 / 436.5 MiB | 435.8 / 413.9 MiB |
| collections | 6, longest 1,302 ms, total 7,396 ms | 6, longest 1,787 ms, total 8,940 ms |

Findings:
* Recycling of for-each iterators was lost: the inliner opened `iterator()` and the iterator constructors, so the shapes that `find_iterator_scopes` / `recycle_slot` match were gone. Fixed afterwards (`iterator()` calls and recyclable constructors stay calls) and covered by `recycle_tests.py` with `--inline --escape`. Not yet measured in the game.
* Frame-allocated objects are not counted by the profiler; their effect shows only as lower totals (BlockPos, Vec3).
* Every collection stops the game for 1.3-1.8 s, about every 31 s (heap 1.17 GiB, live ~500 MiB). This is the number the idle collector (`GC_PLAN.md`) must remove.
* Live bytes after a collection grow ~2 MiB per 30 s (492 -> 501 MiB in 150 s): to be split into chunk loading and real growth by the leak tool.
* The environment has no sound device and no network (OpenAL device and Realms errors in the log): not regressions.
