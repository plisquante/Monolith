// System information without oshi (and therefore without JNA).
//
// Minecraft reads hardware and process data through oshi in three places: the crash/system report (SystemReport, the
// "hardware" and "software" groups), the F3 screen's CPU line (DebugEntrySystemSpecs.getCpuInfo) and the server metrics
// profiler (ServerMetricsSamplersProvider$CpuStats). oshi reaches the OS through JNA: about 560 methods and the 55 JNA
// natives emulated in monolith/rt/natives_jni.rs. These patches read the same facts directly (Linux: /proc; other
// systems: what the Rust standard library knows) and the originals leave the program ("replace": true).
//
// Differences from oshi, all in report text only, none in game behaviour:
// - Graphics cards and memory slots are not listed (oshi needs lshw/dmidecode for them on Linux and usually gets nothing
//   without root either). The renderer's own GL/Vulkan report lines are unaffected.
// - On macOS and Windows only the logical CPU count is known for now; the other values read "unknown".
// - Process Loads is the process's CPU time over its run time, as oshi's cumulative load.

use std::collections::HashSet;

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// /proc/cpuinfo fields of the first processor, the logical count, packages and physical cores.
struct Cpu {
    vendor: String,
    name: String,
    family: String,
    model: String,
    stepping: String,
    mhz: f64,
    logical: i32,
    packages: i32,
    physical: i32,
}

fn cpu() -> Cpu {
    let logical_std = std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(1);
    let mut c = Cpu { vendor: "unknown".into(), name: "unknown".into(), family: String::new(), model: String::new(),
                      stepping: String::new(), mhz: 0.0, logical: 0, packages: 0, physical: 0 };
    let text = read("/proc/cpuinfo");
    let mut pkgs = HashSet::new();
    let mut cores = HashSet::new();
    let mut cur_pkg = String::new();
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "processor" => c.logical += 1,
            "vendor_id" if c.logical == 1 => c.vendor = v.to_string(),
            "model name" if c.logical == 1 => c.name = v.to_string(),
            "cpu family" if c.logical == 1 => c.family = v.to_string(),
            "model" if c.logical == 1 => c.model = v.to_string(),
            "stepping" if c.logical == 1 => c.stepping = v.to_string(),
            "cpu MHz" if c.logical == 1 => c.mhz = v.parse().unwrap_or(0.0),
            "physical id" => { cur_pkg = v.to_string(); pkgs.insert(cur_pkg.clone()); }
            "core id" => { cores.insert((cur_pkg.clone(), v.to_string())); }
            _ => {}
        }
    }
    if c.logical == 0 {
        c.logical = logical_std;
    }
    c.packages = if pkgs.is_empty() { 1 } else { pkgs.len() as i32 };
    c.physical = if cores.is_empty() { c.logical } else { cores.len() as i32 };
    c
}

fn identifier(c: &Cpu) -> String {
    if c.family.is_empty() {
        return "unknown".into();
    }
    let arch = if cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64") { "64" } else { "" };
    let fam = if c.vendor.contains("Intel") { format!("Intel{arch}") } else if c.vendor.contains("AMD") { format!("AMD{arch}") } else { c.vendor.clone() };
    format!("{fam} Family {} Model {} Stepping {}", c.family, c.model, c.stepping)
}

/// /proc/meminfo value in bytes (the file is in KiB).
fn meminfo(key: &str) -> Option<u64> {
    read("/proc/meminfo").lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        if k.trim() != key { return None; }
        v.split_whitespace().next()?.parse::<u64>().ok().map(|kb| kb * 1024)
    })
}

fn mib(bytes: u64) -> String {
    format!("{:.2}", bytes as f64 / 1024.0 / 1024.0)
}

fn opt_mib(v: Option<u64>) -> String {
    v.map(mib).unwrap_or_else(|| "unknown".into())
}

fn detail(this: rt::Ref, key: &str, val: String) -> rt::Res<()> {
    v_set_detail(this, rt::new_str(key.to_string()), rt::new_str(val))
}

/// SystemReport::lambda$new$6, the "hardware" group (the original does `putHardware(new SystemInfo())`).
pub fn hardware(this: rt::Ref) -> rt::Res<()> {
    let c = cpu();
    detail(this, "Processor Vendor", c.vendor.clone())?;
    detail(this, "Processor Name", c.name.clone())?;
    detail(this, "Identifier", identifier(&c))?;
    detail(this, "Microarchitecture", "unknown".into())?;
    detail(this, "Frequency (GHz)", format!("{:.2}", c.mhz / 1000.0))?;
    detail(this, "Number of physical packages", c.packages.to_string())?;
    detail(this, "Number of physical CPUs", c.physical.to_string())?;
    detail(this, "Number of logical CPUs", c.logical.to_string())?;
    let commit_limit = meminfo("CommitLimit");
    let committed = meminfo("Committed_AS");
    let swap_total = meminfo("SwapTotal");
    let swap_used = match (swap_total, meminfo("SwapFree")) { (Some(t), Some(f)) => Some(t.saturating_sub(f)), _ => None };
    detail(this, "Virtual memory max (MiB)", opt_mib(commit_limit))?;
    detail(this, "Virtual memory used (MiB)", opt_mib(committed))?;
    detail(this, "Swap memory total (MiB)", opt_mib(swap_total))?;
    detail(this, "Swap memory used (MiB)", opt_mib(swap_used))?;
    c_put_storage(this)
}

fn os_version() -> String {
    let os = read("/etc/os-release");
    let pretty = os.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|v| v.trim_matches('"').to_string());
    let kernel = read("/proc/sys/kernel/osrelease").trim().to_string();
    match (pretty, kernel.is_empty()) {
        (Some(p), false) => format!("GNU/Linux {p} build {kernel}"),
        (Some(p), true) => format!("GNU/Linux {p}"),
        (None, false) => format!("{} build {kernel}", std::env::consts::OS),
        (None, true) => std::env::consts::OS.to_string(),
    }
}

extern "C" {
    fn geteuid() -> u32;
    fn sysconf(name: i32) -> i64;
}

/// SystemReport::lambda$new$7, the "software" group (`putSoftware(new SystemInfo())`).
pub fn software(this: rt::Ref) -> rt::Res<()> {
    detail(this, "Operating System Version", os_version())?;
    let elevated = cfg!(unix) && unsafe { geteuid() } == 0;
    detail(this, "Process Elevated", elevated.to_string())?;
    // /proc/self/stat: utime 14, stime 15, starttime 22, vsize 23, rss 24 (1-based, after the command name)
    let stat = read("/proc/self/stat");
    let fields: Vec<&str> = stat.rsplit_once(')').map(|(_, r)| r.split_whitespace().collect()).unwrap_or_default();
    let f = |i: usize| fields.get(i - 3).and_then(|v| v.parse::<u64>().ok());
    let hz = unsafe { sysconf(2) }.max(1) as f64;          // _SC_CLK_TCK
    let page = unsafe { sysconf(30) }.max(1) as u64;      // _SC_PAGESIZE
    let uptime: f64 = read("/proc/uptime").split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let load = match (f(14), f(15), f(22)) {
        (Some(u), Some(s), Some(st)) => {
            let run = uptime - st as f64 / hz;
            if run > 0.0 { format!("{:.2}%", (u + s) as f64 / hz / run * 100.0) } else { "unknown".into() }
        }
        _ => "unknown".into(),
    };
    detail(this, "Process Loads", load)?;
    detail(this, "Process Virtual Size (MiB)", opt_mib(f(23)))?;
    detail(this, "Process Resident Size (MiB)", opt_mib(f(24).map(|p| p * page)))?;
    Ok(())
}

/// DebugEntrySystemSpecs.getCpuInfo: "<logical>x <processor name>", whitespace runs collapsed, computed once.
pub fn cpu_info() -> rt::Res<rt::Ref> {
    unsafe {
        let cur: rt::Ref = *s_cpu_info();
        if !cur.is_null() {
            return Ok(cur);
        }
        let c = cpu();
        let s = format!("{}x {}", c.logical, c.name).split_whitespace().collect::<Vec<_>>().join(" ");
        let r = rt::new_str(s);
        *s_cpu_info() = r;
        Ok(r)
    }
}

/// Per-CPU busy and total ticks from /proc/stat (oshi: busy = total - idle - iowait).
fn cpu_ticks() -> Vec<(u64, u64)> {
    read("/proc/stat").lines().filter(|l| l.starts_with("cpu") && l.as_bytes().get(3).is_some_and(|b| b.is_ascii_digit()))
        .map(|l| {
            let v: Vec<u64> = l.split_whitespace().skip(1).take(8).map(|x| x.parse().unwrap_or(0)).collect();
            let total: u64 = v.iter().sum();
            let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
            (total.saturating_sub(idle), total)
        }).collect()
}

struct CpuState { prev: Vec<(u64, u64)>, load: Vec<f64>, last_poll_ms: i64 }

fn states() -> &'static std::sync::Mutex<std::collections::HashMap<usize, CpuState>> {
    static S: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<usize, CpuState>>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// ServerMetricsSamplersProvider$CpuStats.<init>: nrOfCpus and the first tick snapshot (loads start at 0).
pub fn cpu_stats_init(this: rt::Ref) -> rt::Res<()> {
    let ticks = cpu_ticks();
    let n = if ticks.is_empty() { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) } else { ticks.len() };
    unsafe { set::<i32>(this, F_nr_of_cpus, n as i32) };
    states().lock().unwrap().insert(this.0 as usize, CpuState { prev: ticks, load: vec![0.0; n], last_poll_ms: 0 });
    Ok(())
}

/// ServerMetricsSamplersProvider$CpuStats.loadForCpu: percent busy of one CPU, refreshed at most every 501 ms.
pub fn cpu_stats_load(this: rt::Ref, cpu: i32) -> rt::Res<f64> {
    let now = now_ms();
    let mut map = states().lock().unwrap();
    let Some(st) = map.get_mut(&(this.0 as usize)) else {
        return rt::throw("java/lang/NullPointerException", String::new());
    };
    if st.last_poll_ms == 0 || st.last_poll_ms + 501 < now {
        let cur = cpu_ticks();
        st.load = st.prev.iter().zip(cur.iter()).map(|(p, c)| {
            let dt = c.1.saturating_sub(p.1);
            if dt == 0 { 0.0 } else { c.0.saturating_sub(p.0) as f64 / dt as f64 }
        }).collect();
        if st.load.is_empty() {
            st.load = vec![0.0; st.prev.len().max(1)];
        }
        st.prev = cur;
        st.last_poll_ms = now;
    }
    match st.load.get(cpu as usize) {
        Some(v) if cpu >= 0 => Ok(v * 100.0),
        _ => rt::throw("java/lang/ArrayIndexOutOfBoundsException",
                       format!("Index {} out of bounds for length {}", cpu, st.load.len())),
    }
}

/// NativeModuleLister.listModules: the list of loaded native modules for crash reports. The original returns an empty
/// list everywhere but Windows, where it walks the modules through JNA (Kernel32). This build has no JNA, so the list is
/// empty on every platform (crash reports on Windows lose the "Native Modules" lines).
pub fn no_modules() -> rt::Res<rt::Ref> {
    c_empty_list()
}
