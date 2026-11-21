//! `java.util.Random` (JDK) algoritmasının bit-birebir yeniden uygulaması.
//!
//! Algoritma JDK belgelerindeki LCG tanımıdır: 48-bitlik durum, çarpan
//! `0x5DEECE66D`, toplama `0xB`. Her metodun davranışı JDK 21 ile
//! `tests/fixtures/java_random.txt` üzerinden karşılaştırılır.
//!
//! Bu tip Minecraft'ın vanilla `LegacyRandomSource` davranışıyla aynı olduğu
//! varsayılarak değil, yalnızca JDK ile eşleştiği test edilerek kabul edilir.
//! Vanilla eşdeğerliği ayrıca doğrulanmadı.

const MULTIPLIER: u64 = 0x5_DEEC_E66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = (1u64 << 48) - 1;

/// JDK `java.util.Random` ile aynı çıktı dizisini üreten deterministik RNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    /// JDK `new Random(seed)` ile aynı başlangıç durumu.
    #[must_use]
    pub const fn new(seed: i64) -> Self {
        Self {
            seed: (seed as u64 ^ MULTIPLIER) & MASK,
        }
    }

    /// JDK `setSeed(seed)` ile aynı.
    pub const fn set_seed(&mut self, seed: i64) {
        self.seed = (seed as u64 ^ MULTIPLIER) & MASK;
    }

    /// JDK `protected int next(int bits)` ile aynı. `bits` 1..=32 olmalı.
    fn next(&mut self, bits: u32) -> i32 {
        debug_assert!((1..=32).contains(&bits));
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        // JDK: (int)(seed >>> (48 - bits)). Üst bitler `as i32` ile kesilir, Java cast'i gibi.
        (self.seed >> (48 - bits)) as i32
    }

    /// JDK `nextInt()`.
    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// JDK `nextInt(int bound)`. `bound > 0` olmalı; JDK'daki `IllegalArgumentException` burada `assert` olur.
    pub fn next_int_below(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
            // Power of two: JDK (int)((bound * (long) r) >> 31)
            r = ((i64::from(bound) * i64::from(r)) >> 31) as i32;
        } else {
            // JDK: for (int u = r; u - (r = u % bound) + m < 0; u = next(31));
            // Java'da int taşması vardır; wrapping aritmetik ile aynı davranış.
            let mut u = r;
            loop {
                r = u % bound;
                if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                    break;
                }
                u = self.next(31);
            }
        }
        r
    }

    /// JDK `nextLong()`: `((long) next(32) << 32) + next(32)`.
    pub fn next_long(&mut self) -> i64 {
        let hi = i64::from(self.next(32));
        let lo = i64::from(self.next(32));
        (hi << 32).wrapping_add(lo)
    }

    /// JDK `nextBoolean()`.
    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    /// JDK `nextFloat()`: `next(24) / (float)(1 << 24)`.
    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1u32 << 24) as f32
    }

    /// JDK `nextDouble()`: `(((long) next(26) << 27) + next(27)) * 2^-53`.
    pub fn next_double(&mut self) -> f64 {
        let hi = i64::from(self.next(26));
        let lo = i64::from(self.next(27));
        let bits = (hi << 27).wrapping_add(lo);
        bits as f64 * (1.0f64 / (1u64 << 53) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::JavaRandom;

    /// JDK 21 ile elde edilen bilinen değerler (`new Random(42).nextInt()`, `new Random(0).nextInt()`).
    #[test]
    fn known_first_values() {
        assert_eq!(JavaRandom::new(42).next_int(), -1_170_105_035);
        assert_eq!(JavaRandom::new(0).next_int(), -1_155_484_576);
    }

    #[test]
    fn set_seed_matches_new() {
        let mut a = JavaRandom::new(7);
        let _ = a.next_long();
        a.set_seed(7);
        assert_eq!(a, JavaRandom::new(7));
    }

    /// JDK çıktısıyla bit-birebir karşılaştırma. Fixture `tools/oracle/JavaRandomOracle.java` ile üretildi.
    #[test]
    fn matches_jdk_fixture_bit_for_bit() {
        const FIXTURE: &str = include_str!("../tests/fixtures/java_random.txt");
        let mut current: Option<(i64, JavaRandom)> = None;
        let mut checked = 0usize;

        for (lineno, line) in FIXTURE.lines().enumerate() {
            let mut parts = line.split_whitespace();
            let seed: i64 = parts.next().unwrap().parse().unwrap();
            let op = parts.next().unwrap();
            let arg: i64 = parts.next().unwrap().parse().unwrap();
            let expected: i64 = parts.next().unwrap().parse().unwrap();

            if current.as_ref().map(|(s, _)| *s) != Some(seed) {
                current = Some((seed, JavaRandom::new(seed)));
            }
            let rng = &mut current.as_mut().unwrap().1;

            let got: i64 = match op {
                "I" => i64::from(rng.next_int()),
                "B" => i64::from(rng.next_int_below(i32::try_from(arg).unwrap())),
                "L" => rng.next_long(),
                "Z" => i64::from(rng.next_boolean()),
                "F" => i64::from(rng.next_float().to_bits() as i32),
                "D" => rng.next_double().to_bits() as i64,
                other => panic!("bilinmeyen op {other} satır {}", lineno + 1),
            };
            assert_eq!(got, expected, "satır {}: {line}", lineno + 1);
            checked += 1;
        }

        assert_eq!(checked, 2400, "fixture satır sayısı beklenenden farklı");
    }
}
