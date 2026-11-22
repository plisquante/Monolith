// DataFixerUpper warm-up. `DataFixers.optimize(Set)` pre-computes the rewrite rules of every fixer at start-up:
// about 1.4 million rule objects per rule class (roughly 0.6 GiB that stays alive for the whole session) and the
// allocation that builds them. The rules are only needed to upgrade data written by an older game version, and
// DataFixerUpper computes them on first use when they were not optimized. A current world never needs them.
//
// The original runs when MONO_DATAFIX_OPTIMIZE=1 (e.g. to measure the first-load cost of old worlds).

fn keep_original() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MONO_DATAFIX_OPTIMIZE").map_or(false, |v| !v.is_empty() && v != "0"))
}

pub fn optimize(orig: impl Fn(rt::Ref) -> rt::Res<rt::Ref>, types: rt::Ref) -> rt::Res<rt::Ref> {
    if keep_original() {
        return orig(types);
    }
    c_completed(rt::Ref::NULL)
}
