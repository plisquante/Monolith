// Prototip: eşzamanlı SATB (snapshot-at-the-beginning) işaretleme. Kendi başına derlenir:
//   rustc --edition 2021 --test tests/rust/satb_proto.rs -o /tmp/satb && /tmp/satb
//
// Model: düğümler bir dizide, her düğümün bir "alan" referansı var (0 = boş). Bir işaretleyici thread, kökten başlayıp
// başlangıç anlık görüntüsündeki her düğümü işaretler. Mutatörler işaretleme sürerken referansları değiştirir; her
// yazmadan önce eski değeri SATB tamponuna iterler (bariyer). Doğruluk: işaretleme bittiğinde, anlık görüntüde canlı
// olan her düğüm işaretli olmalıdır (kaybolan nesne yok).
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const N: usize = 4096;

pub struct Heap {
    // refs[i] = i nesnesinin gösterdiği nesnenin indeksi (usize::MAX = yok). Kökler: root[0..R].
    refs: Vec<AtomicUsize>,
    marks: Vec<AtomicU32>,
    marking: AtomicBool,
    satb: Mutex<Vec<usize>>,
}

const NONE: usize = usize::MAX;

impl Heap {
    fn new() -> Heap {
        Heap {
            refs: (0..N).map(|_| AtomicUsize::new(NONE)).collect(),
            marks: (0..N).map(|_| AtomicU32::new(0)).collect(),
            marking: AtomicBool::new(false),
            satb: Mutex::new(Vec::new()),
        }
    }

    /// Yazma bariyeri: `refs[obj] = new`. Eski değer işaretleme sürerken tampona gider.
    fn write(&self, obj: usize, new: usize) {
        let old = self.refs[obj].load(Ordering::Acquire);
        if self.marking.load(Ordering::Acquire) && old != NONE {
            self.satb.lock().unwrap().push(old);
        }
        self.refs[obj].store(new, Ordering::Release);
    }

    fn mark_one(&self, v: usize, stack: &mut Vec<usize>) {
        if v != NONE && self.marks[v].swap(1, Ordering::AcqRel) == 0 {
            stack.push(v);
        }
    }

    /// İşaretleyici: kökler ve tampon boşaltma, kuyruk boşalana kadar.
    fn mark(&self, roots: &[usize]) {
        let mut stack = Vec::new();
        for &r in roots {
            self.mark_one(r, &mut stack);
        }
        loop {
            while let Some(o) = stack.pop() {
                let next = self.refs[o].load(Ordering::Acquire);
                self.mark_one(next, &mut stack);
            }
            // tamponu boşalt (mutatörlerin eski değerleri)
            let drained: Vec<usize> = std::mem::take(&mut *self.satb.lock().unwrap());
            if drained.is_empty() {
                break;
            }
            for v in drained {
                self.mark_one(v, &mut stack);
            }
        }
    }
}

#[test]
fn snapshot_objects_survive_concurrent_mutation() {
    for trial in 0..50u64 {
        let heap = Arc::new(Heap::new());
        // başlangıç grafı: zincirler (her düğüm bir sonrakini gösterir), kökler 0..16
        let mut seed = 0x9E37_79B9_7F4A_7C15u64 ^ trial;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for i in 0..N {
            let t = (rnd() as usize) % N;
            heap.refs[i].store(if rnd() % 4 == 0 { NONE } else { t }, Ordering::Relaxed);
        }
        let roots: Vec<usize> = (0..16).collect();
        // anlık görüntüde (işaretleme başlamadan) canlı olanlar: kökten erişilebilenler
        let snapshot = {
            let mut seen = vec![false; N];
            let mut st: Vec<usize> = roots.clone();
            for &r in &roots { seen[r] = true; }
            while let Some(o) = st.pop() {
                let n = heap.refs[o].load(Ordering::Relaxed);
                if n != NONE && !seen[n] { seen[n] = true; st.push(n); }
            }
            seen
        };
        heap.marking.store(true, Ordering::SeqCst);
        std::thread::scope(|s| {
            // mutatörler: işaretleme sürerken rastgele referans değiştirirler
            for m in 0..4u64 {
                let h = heap.clone();
                s.spawn(move || {
                    let mut x = 0xDEAD_BEEFu64 ^ (trial << 8) ^ m;
                    for _ in 0..20_000 {
                        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
                        let obj = (x as usize) % N;
                        let new = if x % 3 == 0 { NONE } else { ((x >> 20) as usize) % N };
                        h.write(obj, new);
                    }
                });
            }
            // işaretleyici
            let h = heap.clone();
            let roots2 = roots.clone();
            s.spawn(move || h.mark(&roots2));
        });
        // Mutatörler bitti; işaretleme de bitmiş olmalı (tek başına dolaşım tamamlandı)
        for i in 0..N {
            if snapshot[i] {
                assert_eq!(heap.marks[i].load(Ordering::Relaxed), 1,
                    "trial {trial}: object {i} was reachable at the snapshot but not marked (lost)");
            }
        }
    }
}
