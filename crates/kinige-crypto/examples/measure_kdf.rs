//! Argon2id wall time and peak memory of a profile (ADR-0001 decision 8: "measure in S4b on the weakest machine"; the limit to discuss is
//! 2 s). Run in release, one profile per process so that the peak memory is that profile's:
//!
//! ```text
//! cargo run --release -p kinige-crypto --example measure_kdf -- 1 5    # profile 1, 5 runs
//! cargo run --release -p kinige-crypto --example measure_kdf -- 2 1    # profile 2, 1 run
//! ```
//!
//! The password and the salt are fixed and fake. Peak memory is the process's peak resident set (`ru_maxrss` on Unix, `PeakWorkingSetSize`
//! on Windows), which includes the program itself (a few MiB).
// A measurement program, not app code: it prints, indexes a vector it just filled (`runs >= 1`) and stops on a failed derivation.
#![allow(
    unsafe_code,
    clippy::print_stdout,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use kinige_crypto::Profile;
use kinige_crypto::keys::derive_pw_root;
use kinige_crypto::secret::SecretBytes;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let profile = match args.next().as_deref() {
        Some("2") => Profile::P2,
        _ => Profile::P1,
    };
    let runs: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(5).max(1);
    let password = SecretBytes::new(b"fake password for the measurement".to_vec());
    let salt = [0x22u8; 16];

    let mut millis: Vec<f64> = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        let root = derive_pw_root(&password, &salt, profile).expect("derive");
        millis.push(start.elapsed().as_secs_f64() * 1000.0);
        drop(root);
    }
    millis.sort_by(f64::total_cmp);
    let median = millis[millis.len() / 2];
    println!(
        "profile {} ({} passes, {} MiB): {} run(s), min {:.0} ms, median {:.0} ms, max {:.0} ms, peak memory {}",
        profile.number(),
        profile.ops_limit(),
        profile.mem_limit() >> 20,
        runs,
        millis[0],
        median,
        millis[millis.len() - 1],
        peak_memory().map_or("not measured".to_string(), |bytes| format!(
            "{} MiB",
            bytes >> 20
        )),
    );
}

#[cfg(unix)]
fn peak_memory() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `getrusage` writes a `rusage` into the pointer it is given, which is valid for that.
    let usage = unsafe {
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
            return None;
        }
        usage.assume_init()
    };
    let max = u64::try_from(usage.ru_maxrss).ok()?;
    // Bytes on macOS, kilobytes on Linux.
    Some(if cfg!(target_os = "macos") {
        max
    } else {
        max * 1024
    })
}

#[cfg(windows)]
fn peak_memory() -> Option<u64> {
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(
            process: isize,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }
    let mut counters = ProcessMemoryCounters {
        cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
    };
    // SAFETY: the struct is `#[repr(C)]` as PROCESS_MEMORY_COUNTERS and `cb` is its size; the pseudo handle of the current process is valid.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    (ok != 0).then_some(counters.peak_working_set_size as u64)
}

#[cfg(not(any(unix, windows)))]
fn peak_memory() -> Option<u64> {
    None
}
