// Hand-written fast paths for the "world height" idiom that runs on every block access:
//   ChunkAccess.getMinY -> LevelHeightAccessor.getMinY -> LevelReader.getMinY -> Level.dimensionType
//   -> Holder.value -> DimensionType.minY
// which is five calls (three of them virtual) in the translated code. Each patch handles only the common
// receiver classes (a client or server level, a resolved holder) and passes everything else (other
// LevelReader implementations such as WorldGenRegion, unbound holders, wrong types) to the translated original.
const rt = @import("rt.zig");
const P = @import("classes.zig");
const Ref = rt.Ref;
const Holder = P.C_net_minecraft_core_Holder_2Reference;
const Level = P.C_net_minecraft_world_level_Level;
const DimType = P.C_net_minecraft_world_level_dimension_DimensionType;
const ClientLevel = P.C_net_minecraft_client_multiplayer_ClientLevel;
const ServerLevel = P.C_net_minecraft_server_level_ServerLevel;

/// minY of the dimension type of `o` when `o` is exactly a client or server level with a bound holder.
inline fn levelMinY(o: *rt.Object) ?i32 {
    const c = o.class;
    if (c != ClientLevel.cls() and c != ServerLevel.cls()) return null;
    const h = Level.cast(o).f_dimensionTypeRegistration orelse return null;
    if (h.class != Holder.cls()) return null;
    const v = Holder.cast(h).f_value orelse return null;
    if (v.class != DimType.cls()) return null;
    return DimType.cast(v).f_minY;
}

pub fn holderValue(orig: anytype, this: Ref) rt.E!Ref {
    const o = this orelse return orig(this);
    return Holder.cast(o).f_value orelse orig(this);
}

pub fn levelDimensionType(orig: anytype, this: Ref) rt.E!Ref {
    const o = this orelse return orig(this);
    const h = Level.cast(o).f_dimensionTypeRegistration orelse return orig(this);
    if (h.class == Holder.cls()) {
        if (Holder.cast(h).f_value) |v| {
            if (v.class == DimType.cls()) return v;
        }
    }
    return orig(this);
}

pub fn readerMinY(orig: anytype, this: Ref) rt.E!i32 {
    const o = this orelse return orig(this);
    return levelMinY(o) orelse orig(this);
}

pub fn chunkMinY(orig: anytype, this: Ref) rt.E!i32 {
    const o = this orelse return orig(this);
    const a = P.C_net_minecraft_world_level_chunk_ChunkAccess.cast(o).f_levelHeightAccessor orelse return orig(this);
    return levelMinY(a) orelse orig(this);
}
