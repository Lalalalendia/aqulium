//! RAM baseline experiment using Aquilum's real Rust core and CRDT/Yrs.
#![cfg(windows)]
use crate::app_core::{Core, CoreEvent};
use std::ffi::c_void;
use std::fs;
use std::sync::Arc;
use std::path::PathBuf;

#[repr(C)]
#[derive(Default)]
struct ProcessMemoryCountersEx {
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
    private_usage: usize,
}
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
}
#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(process: *mut c_void, counters: *mut ProcessMemoryCountersEx, cb: u32) -> i32;
}

fn snapshot(phase: &str, bytes: usize) {
    let mut x = ProcessMemoryCountersEx {
        cb: std::mem::size_of::<ProcessMemoryCountersEx>() as u32,
        ..Default::default()
    };
    let size = x.cb;
    let success = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut x, size) };
    assert_ne!(success, 0, "Windows GetProcessMemoryInfo failed");
    let mib = |n: usize| n as f64 / 1_048_576.0;
    println!(
        "AQUILUM_CORE_RAM,{phase},{bytes},{:.3},{:.3},{:.3}",
        mib(x.working_set_size), mib(x.private_usage), mib(x.peak_working_set_size)
    );
}

#[test]
fn native_ram_profile() {
    println!("AQUILUM_CORE_RAM_HEADER,phase,note_size_bytes,working_set_mib,private_commit_mib,peak_working_set_mib");
    let temp = tempfile::tempdir().expect("tempdir");
    let vault = temp.path().join("vault");
    let data = temp.path().join("appdata");
    fs::create_dir_all(&vault).unwrap();
    fs::create_dir_all(&data).unwrap();
    let note = vault.join("Benchmark.md");
    let line = "## Chapter\nRussian and Latin text, local-only benchmark. Some words, more words.\n";
    let body = line.repeat(5_000_000 / line.len() + 1);
    fs::write(&note, &body).unwrap();
    let expected = body.len();
    drop(body);
    let core = Core::open(&data, Arc::new(|_: CoreEvent| {}));
    snapshot("core_initialized", expected);
    let first = core.documents.open(&core, &note, 0).unwrap();
    assert_eq!(first.text.len(), expected);
    snapshot("one_5mb_open", expected);
    core.documents.release(&core, &note);
    snapshot("one_5mb_closed", expected);
    for _ in 0..50 {
        let opened = core.documents.open(&core, &note, 0).unwrap();
        assert_eq!(opened.text.len(), expected);
        core.documents.release(&core, &note);
    }
    snapshot("after_50_reopens", expected);
    let mut open = Vec::<PathBuf>::new();
    for i in 0..5 {
        let path = vault.join(format!("Copy-{i}.md"));
        fs::copy(&note, &path).unwrap();
        let loaded = core.documents.open(&core, &path, 0).unwrap();
        assert_eq!(loaded.text.len(), expected);
        open.push(path);
    }
    snapshot("five_more_5mb_open", expected);
    for path in &open { core.documents.release(&core, path); }
    snapshot("five_more_5mb_closed", expected);
    core.shutdown();
    snapshot("after_shutdown", expected);
}
