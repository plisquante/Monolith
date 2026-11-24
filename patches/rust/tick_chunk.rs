// ServerLevel.tickChunk: bir yongaya ait rastgele tick, çöp üretmeden.
//
// Çevrilmiş metot, her tick atan yonga bölümünün her rastgele tick konumu için ve her oyun tick'inde getBlockRandomPos çağırır
// (her seferinde yeni bir BlockPos; sunucunun ayırdığı her şeyin yaklaşık beşte biri), içinden üç tamsayı okur ve
// onu bırakır. Bu blokların neredeyse hiçbiri rastgele tick atmaz, yani nesne neredeyse hiç kullanılmaz. Bu sürüm
// konumu tamsayı olarak hesaplar ve BlockPos'u yalnızca gerçekten tick atan blok (ya da sıvı) için ayırır.
//
// Java metoduyla aynı davranış: rastgele sayıların aynı sırası (seviyenin `randValue` üreteci, Level.getBlockRandomPos'un
// yaptığı gibi tam olarak ilerletilir), aynı profilci bölümleri, aynı istisnalar (null yonga ya da bölüm, aynı çağrıda
// NullPointerException'dır).

/// Nesne olmadan Level.getBlockRandomPos: `randValue`'yu ilerletir ve konumun koordinatlarını döndürür.
#[inline(always)]
fn random_pos(this: rt::Ref, x: i32, y: i32, z: i32, mask: i32) -> (i32, i32, i32) {
    unsafe {
        let rv = get::<i32>(this, F_level_rand_value).wrapping_mul(3).wrapping_add(1013904223);
        set::<i32>(this, F_level_rand_value, rv);
        let i = rv >> 2;
        (x.wrapping_add(i & 15), y.wrapping_add((i >> 16) & mask), z.wrapping_add((i >> 8) & 15))
    }
}

pub fn tick_chunk(this: rt::Ref, chunk: rt::Ref, speed: i32) -> rt::Res<()> {
    let chunk_pos = v_chunk_pos(chunk)?;
    rt::nn(chunk_pos)?;
    let min_x = unsafe { get::<i32>(chunk_pos, F_chunkpos_x) }.wrapping_shl(4); // ChunkPos.getMinBlockX
    let min_z = unsafe { get::<i32>(chunk_pos, F_chunkpos_z) }.wrapping_shl(4);
    let prof = c_profiler_get()?;
    v_prof_push(prof, str_iceandsnow())?;
    let mut i = 0;
    while i < speed {
        let random: rt::Ref = unsafe { get(this, F_level_random) };
        if v_next_int(random, 48)? == 0 {
            let (x, y, z) = random_pos(this, min_x, 0, min_z, 15);
            let pos = n_block_pos(x, y, z)?;
            v_tick_precipitation(this, pos)?;
        }
        i += 1;
    }
    v_prof_pop_push(prof, str_tick_blocks())?;
    if speed > 0 {
        let sections = v_chunk_sections(chunk)?;
        rt::nn(sections)?;
        let n = unsafe { array_len(sections) };
        let mut si = 0;
        while si < n {
            let section: rt::Ref = unsafe { array::<rt::Ref>(sections)[si as usize] };
            if v_section_ticking(section)? != 0 {
                let base_y = v_section_y(chunk, si)?.wrapping_shl(4); // SectionPos.sectionToBlockCoord
                let mut k = 0;
                while k < speed {
                    let (x, y, z) = random_pos(this, min_x, base_y, min_z, 15);
                    v_prof_push(prof, str_random_tick())?;
                    let state = v_section_state(section, x.wrapping_sub(min_x), y.wrapping_sub(base_y), z.wrapping_sub(min_z))?;
                    let mut pos = rt::Ref::NULL;
                    if v_state_ticking(state)? != 0 {
                        pos = n_block_pos(x, y, z)?;
                        let random: rt::Ref = unsafe { get(this, F_level_random) };
                        v_state_random_tick(state, this, pos, random)?;
                    }
                    let fluid = v_state_fluid(state)?;
                    if v_fluid_ticking(fluid)? != 0 {
                        if pos.is_null() {
                            pos = n_block_pos(x, y, z)?;
                        }
                        let random: rt::Ref = unsafe { get(this, F_level_random) };
                        v_fluid_random_tick(fluid, this, pos, random)?;
                    }
                    v_prof_pop(prof)?;
                    k += 1;
                }
            }
            si += 1;
        }
    }
    v_prof_pop(prof)
}
