//! Paralel yonga ağı oluşturma, eleme ve asset çözme için işçi havuzu. İşçiler boştayken bir koşul değişkeninde park eder.
//! Görevler, opak bir bağlamla düz fonksiyon işaretçileridir; bu yüzden birini göndermek hiçbir şey ayırmaz:
//! kuyruk `start` sırasında bir kez boyutlandırılır.
use std::collections::VecDeque;
use std::ffi::c_void;
use std::io;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

/// Görev gövdesi: `ctx` çağıranın bağlamı, `arg` bir toplu işin dilim indeksidir.
pub type TaskFn = unsafe fn(ctx: *mut c_void, arg: usize);

pub const QUEUE_CAP: usize = 4096;
const MAX_WORKERS: usize = 16;
const WORKER_STACK: usize = 512 * 1024;

/// Tamamlanma bariyeri: `wait`, ona sayılmış her görev `done` çağırdığında döner.
pub struct WaitGroup {
    pending: AtomicUsize,
    lock: Mutex<()>,
    cv: Condvar,
}

impl WaitGroup {
    pub const fn new() -> Self {
        WaitGroup { pending: AtomicUsize::new(0), lock: Mutex::new(()), cv: Condvar::new() }
    }

    pub fn add(&self, count: usize) {
        self.pending.fetch_add(count, Ordering::AcqRel);
    }

    pub fn done(&self) {
        if self.pending.fetch_sub(1, Ordering::AcqRel) == 1 {
            let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
            self.cv.notify_all();
        }
    }

    pub fn wait(&self) {
        if self.pending.load(Ordering::Acquire) == 0 {
            return;
        }
        let mut g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        while self.pending.load(Ordering::Acquire) > 0 {
            g = self.cv.wait(g).unwrap_or_else(|e| e.into_inner());
        }
    }
}

impl Default for WaitGroup {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct Task {
    func: TaskFn,
    ctx: *mut c_void,
    arg: usize,
    wg: *const WaitGroup,
}

// İşaretçiler çağıranın sahipliğindedir; `submit` ve `dispatch_batch`, görev çalışana kadar onların geçerli kalmasını gerektirir.
unsafe impl Send for Task {}

struct Queue {
    tasks: VecDeque<Task>,
    stopping: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    cv: Condvar,
    completed: AtomicU64,
}

pub struct WorkerPool {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    pub fn new() -> Self {
        WorkerPool {
            shared: Arc::new(Shared {
                queue: Mutex::new(Queue { tasks: VecDeque::with_capacity(QUEUE_CAP), stopping: false }),
                cv: Condvar::new(),
                completed: AtomicU64::new(0),
            }),
            workers: Vec::new(),
        }
    }

    /// İşçileri başlatır. `requested == 0` CPU sayısından bir eksiği seçer, en fazla 8.
    pub fn start(&mut self, requested: usize) -> io::Result<()> {
        let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
        let n = if requested == 0 {
            cpus.saturating_sub(1).clamp(1, 8)
        } else {
            requested.clamp(1, MAX_WORKERS)
        };
        {
            let mut q = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            q.stopping = false;
            q.tasks.clear();
        }
        for _ in 0..n {
            let shared = Arc::clone(&self.shared);
            let h = std::thread::Builder::new().stack_size(WORKER_STACK).spawn(move || worker_main(&shared))?;
            self.workers.push(h);
        }
        Ok(())
    }

    /// İş kabul etmeyi bırakır, kuyruktakileri çalıştırır ve her işçiyi birleştirir (join).
    pub fn stop(&mut self) {
        {
            let mut q = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            q.stopping = true;
            self.shared.cv.notify_all();
        }
        for h in self.workers.drain(..) {
            let _ = h.join();
        }
    }

    pub fn num_workers(&self) -> usize {
        self.workers.len()
    }

    pub fn completed_tasks(&self) -> u64 {
        self.shared.completed.load(Ordering::Relaxed)
    }

    /// Bir görev gönderir. Kuyruk doluysa ya da işçi yoksa görev çağıranda çalışır.
    ///
    /// # Safety
    /// `ctx` ve `wg` (verilmişse), görev çalışana kadar geçerli kalmalıdır.
    pub unsafe fn submit(&self, func: TaskFn, ctx: *mut c_void, arg: usize, wg: Option<&WaitGroup>) {
        if let Some(w) = wg {
            w.add(1);
        }
        let run_here = {
            let mut q = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            if q.stopping || self.workers.is_empty() || q.tasks.len() == QUEUE_CAP {
                true
            } else {
                let wg_ptr = wg.map_or(std::ptr::null(), |w| w as *const WaitGroup);
                q.tasks.push_back(Task { func, ctx, arg, wg: wg_ptr });
                false
            }
        };
        if run_here {
            func(ctx, arg);
            self.shared.completed.fetch_add(1, Ordering::Relaxed);
            if let Some(w) = wg {
                w.done();
            }
        } else {
            self.shared.cv.notify_one();
        }
    }

    /// `func(ctx, 0..count)` görevlerinin `count` tanesini tek bir kilit altında gönderir, sonra işçileri uyandırır. Kuyruğa sığmayan
    /// her şey çağıranda çalışır.
    ///
    /// # Safety
    /// `ctx`, `wg.wait()` dönene kadar geçerli kalmalıdır.
    pub unsafe fn dispatch_batch(&self, func: TaskFn, ctx: *mut c_void, count: usize, wg: &WaitGroup) {
        if count == 0 {
            return;
        }
        wg.add(count);
        let mut enqueued = 0usize;
        {
            let mut q = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            if !q.stopping && !self.workers.is_empty() {
                while enqueued < count && q.tasks.len() < QUEUE_CAP {
                    q.tasks.push_back(Task { func, ctx, arg: enqueued, wg: wg as *const WaitGroup });
                    enqueued += 1;
                }
            }
        }
        match enqueued {
            0 => {}
            1 => self.shared.cv.notify_one(),
            _ => self.shared.cv.notify_all(),
        }
        for idx in enqueued..count {
            func(ctx, idx);
            self.shared.completed.fetch_add(1, Ordering::Relaxed);
            wg.done();
        }
    }
}

impl Default for WorkerPool {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.stop();
    }
}

fn worker_main(shared: &Shared) {
    loop {
        let task = {
            let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            while q.tasks.is_empty() && !q.stopping {
                q = shared.cv.wait(q).unwrap_or_else(|e| e.into_inner());
            }
            match q.tasks.pop_front() {
                Some(t) => t,
                None => return, // stopping and drained
            }
        };
        // GÜVENLİK: gönderen, görev çalışana kadar `ctx` ve `wg`'yi canlı tutar (bkz. `submit`).
        unsafe { (task.func)(task.ctx, task.arg) };
        shared.completed.fetch_add(1, Ordering::Relaxed);
        if !task.wg.is_null() {
            // GÜVENLİK: yukarıdaki gibi, bariyer görevden uzun yaşar.
            unsafe { (*task.wg).done() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    unsafe fn sum_task(ctx: *mut c_void, idx: usize) {
        let arr = &*(ctx as *const [AtomicU32; 64]);
        arr[idx].store((idx * 3 + 1) as u32, Ordering::Release);
    }

    #[test]
    fn batch_runs_every_slice_once_and_the_fence_releases() {
        let mut pool = WorkerPool::new();
        pool.start(2).expect("spawn workers");
        let results: [AtomicU32; 64] = std::array::from_fn(|_| AtomicU32::new(0));
        let wg = WaitGroup::new();
        unsafe { pool.dispatch_batch(sum_task, &results as *const _ as *mut c_void, 64, &wg) };
        wg.wait();
        for (idx, r) in results.iter().enumerate() {
            assert_eq!(r.load(Ordering::Acquire), (idx * 3 + 1) as u32);
        }
        assert_eq!(pool.completed_tasks(), 64);
        pool.stop();
    }

    #[test]
    fn without_workers_work_runs_on_the_caller() {
        let pool = WorkerPool::new();
        let results: [AtomicU32; 64] = std::array::from_fn(|_| AtomicU32::new(0));
        let wg = WaitGroup::new();
        unsafe { pool.dispatch_batch(sum_task, &results as *const _ as *mut c_void, 64, &wg) };
        wg.wait();
        assert_eq!(results[63].load(Ordering::Acquire), (63 * 3 + 1) as u32);
        assert_eq!(pool.completed_tasks(), 64);
    }
}
