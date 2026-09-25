//! Per-profile resource usage (feature recommendation 2): RAM + CPU of a
//! browser's whole process tree, read straight from `/proc`.
//!
//! Linux-only by design — this is where MBM's orphan detection lives too.
//! Other platforms return an empty list from the command and the UI hides
//! the panel. CPU% is computed as the delta of utime+stime ticks between two
//! samples divided by wall-clock time (kernel CLK_TCK ≈ 100 on every Linux
//! architecture we support).
//!
//! Memory is reported as **PSS** (proportional set size, from
//! `/proc/<pid>/smaps_rollup`), not RSS: Chromium's process tree shares
//! large regions (zygote, shared libraries, V8 snapshots) that every child
//! counts in full in its own RSS — summing RSS over the tree inflates the
//! real footprint ~2×. PSS splits shared pages across the processes that
//! map them, so the tree total matches what the system actually holds.
//! Falls back to RSS for processes without a smaps_rollup (ancient kernels).

use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

/// One running profile's aggregated usage.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUsage {
    pub profile_id: String,
    pub pid: u32,
    /// Memory of the whole process tree in KiB, as PSS (shared pages counted
    /// once across the tree) when the kernel provides smaps_rollup.
    pub memory_kb: u64,
    /// CPU % (all cores summed) since the previous sample; `None` on the
    /// first sample for a pid (no baseline yet).
    pub cpu_percent: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct ProcSample {
    ppid: i32,
    /// utime + stime, in clock ticks.
    cpu_ticks: u64,
    /// Resident pages.
    rss_pages: u64,
}

const PAGE_SIZE_KB: u64 = 4; // x86_64 / aarch64 Linux
const CLK_TCK: f64 = 100.0;

/// Parses the numeric fields we need out of a `/proc/<pid>/stat` line.
/// The comm field (2nd) may contain spaces and parentheses, so parsing starts
/// after the LAST ')'.
fn parse_stat_fields(line: &str) -> Option<ProcSample> {
    let rest = line.rsplit_once(')')?.1.split_whitespace();
    let f: Vec<&str> = rest.collect();
    // After the ')' the state is field 3, so token[i] is field (i + 3).
    let ppid: i32 = f.get(1)?.parse().ok()?; // field 4
    let utime: u64 = f.get(11)?.parse().ok()?; // field 14
    let stime: u64 = f.get(12)?.parse().ok()?; // field 15
    let rss_pages: u64 = f.get(21)?.parse().ok()?; // field 24
    Some(ProcSample {
        ppid,
        cpu_ticks: utime + stime,
        rss_pages,
    })
}

/// Snapshots every process' stat in one /proc scan.
fn read_all_samples() -> HashMap<u32, ProcSample> {
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_string_lossy().parse::<u32>().ok() else {
            continue;
        };
        if let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) {
            if let Some(sample) = parse_stat_fields(&stat) {
                out.insert(pid, sample);
            }
        }
    }
    out
}

/// Extracts the `Pss:` line (in kB) from a `/proc/<pid>/smaps_rollup` body.
fn parse_pss_rollup(body: &str) -> Option<u64> {
    body.lines().find_map(|l| {
        l.strip_prefix("Pss:")
            .and_then(|rest| rest.trim().split(' ').next())
            .and_then(|kb| kb.parse::<u64>().ok())
    })
}

/// Reads a process' PSS in KiB from smaps_rollup; `None` if unavailable.
fn pss_kb(pid: u32) -> Option<u64> {
    std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup"))
        .ok()
        .and_then(|body| parse_pss_rollup(&body))
}

/// Sums cpu ticks + rss over the process tree rooted at `root` (the browser
/// process itself plus every descendant). Returns the visited pids so the
/// caller can do a per-pid PSS pass on top of the cheap stat scan.
fn tree_totals(samples: &HashMap<u32, ProcSample>, root: u32) -> (u64, u64, Vec<u32>) {
    // children[ppid] = [pids]
    let mut children: HashMap<i32, Vec<u32>> = HashMap::new();
    for (&pid, s) in samples {
        children.entry(s.ppid).or_default().push(pid);
    }
    let mut total_cpu = 0u64;
    let mut total_rss = 0u64;
    let mut pids = Vec::new();
    let mut queue = vec![root];
    let mut seen = std::collections::HashSet::new();
    while let Some(pid) = queue.pop() {
        if !seen.insert(pid) {
            continue; // pid cycles are impossible in /proc, but stay safe
        }
        if let Some(s) = samples.get(&pid) {
            total_cpu += s.cpu_ticks;
            total_rss += s.rss_pages;
            pids.push(pid);
            if let Some(kids) = children.get(&(pid as i32)) {
                queue.extend_from_slice(kids);
            }
        }
    }
    (total_cpu, total_rss * PAGE_SIZE_KB, pids)
}

/// Tree memory in KiB: PSS summed over the tree (shared pages counted once),
/// falling back per-pid to RSS where smaps_rollup is unavailable. If the
/// kernel exposes no rollup at all, uses the plain RSS sum.
fn tree_memory_kb(samples: &HashMap<u32, ProcSample>, pids: &[u32], rss_sum_kb: u64) -> u64 {
    let mut total = 0u64;
    let mut any_pss = false;
    for &pid in pids {
        if let Some(pss) = pss_kb(pid) {
            any_pss = true;
            total += pss;
        } else if let Some(s) = samples.get(&pid) {
            total += s.rss_pages * PAGE_SIZE_KB;
        }
    }
    if any_pss { total } else { rss_sum_kb }
}

/// Computes usage for every (profile_id, pid) pair currently running.
/// CPU% baselines are kept in `AppState.usage_samples` between calls.
pub fn compute_usage(state: &AppState, running: Vec<(String, u32)>) -> Vec<ResourceUsage> {
    if running.is_empty() {
        return Vec::new();
    }
    let samples = read_all_samples();
    let now = Instant::now();
    let mut baselines = state
        .usage_samples
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let live_pids: std::collections::HashSet<u32> =
        running.iter().map(|(_, pid)| *pid).collect();
    // Drop baselines of pids that are gone (browser exited/restarted).
    baselines.retain(|pid, _| live_pids.contains(pid));

    running
        .into_iter()
        .map(|(profile_id, pid)| {
            let (cpu_ticks, rss_kb, tree_pids) = tree_totals(&samples, pid);
            let memory_kb = tree_memory_kb(&samples, &tree_pids, rss_kb);
            let cpu_percent = baselines.get(&pid).and_then(|(prev_ticks, prev_at)| {
                let wall = now.duration_since(*prev_at).as_secs_f64();
                if wall < 0.2 {
                    return None; // too short for a meaningful rate
                }
                let d_ticks = cpu_ticks.saturating_sub(*prev_ticks) as f64;
                Some(((d_ticks / CLK_TCK) / wall * 100.0).clamp(0.0, 100.0 * 64.0))
            });
            baselines.insert(pid, (cpu_ticks, now));
            ResourceUsage {
                profile_id,
                pid,
                memory_kb,
                cpu_percent,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stat_line_with_spaces_in_comm() {
        // comm contains spaces/parens — the classic /proc parsing trap.
        // After `)`: state, ppid, then 9 fields (pgrp..cmajflt), utime=25,
        // stime=75, then 8 fields (cutime..vsize), rss=1000 pages.
        let line = "1234 (chrome:crashpad) S 1 0 0 0 0 0 0 0 0 0 25 75 0 0 0 0 0 0 0 0 1000 0";
        let s = parse_stat_fields(line).unwrap();
        assert_eq!(s.ppid, 1);
        assert_eq!(s.cpu_ticks, 100); // utime 25 + stime 75
        assert_eq!(s.rss_pages, 1000);
    }

    #[test]
    fn tree_totals_walk_descendants() {
        let mut samples = HashMap::new();
        samples.insert(
            10u32,
            ProcSample { ppid: 1, cpu_ticks: 10, rss_pages: 100 },
        );
        samples.insert(
            11u32,
            ProcSample { ppid: 10, cpu_ticks: 20, rss_pages: 200 },
        );
        samples.insert(
            12u32,
            ProcSample { ppid: 11, cpu_ticks: 30, rss_pages: 300 },
        );
        samples.insert(
            99u32,
            ProcSample { ppid: 1, cpu_ticks: 999, rss_pages: 9999 },
        );
        let (cpu, mem_kb, pids) = tree_totals(&samples, 10);
        assert_eq!(cpu, 60);
        assert_eq!(mem_kb, (100 + 200 + 300) * PAGE_SIZE_KB);
        assert_eq!(pids, vec![10, 11, 12]);
    }

    #[test]
    fn parses_pss_from_smaps_rollup() {
        let rollup = "Rss:              512000 kB\n\
                      Pss:              128000 kB\n\
                      Pss_Dirty:         64000 kB\n";
        assert_eq!(parse_pss_rollup(rollup), Some(128_000));
        assert_eq!(parse_pss_rollup("Rss: 100 kB"), None);
        assert_eq!(parse_pss_rollup("Pss:      0 kB"), Some(0));
    }

    #[test]
    fn tree_memory_uses_pss_when_available_rss_otherwise() {
        let mut samples = HashMap::new();
        samples.insert(
            10u32,
            ProcSample { ppid: 1, cpu_ticks: 0, rss_pages: 100 },
        );
        samples.insert(
            11u32,
            ProcSample { ppid: 10, cpu_ticks: 0, rss_pages: 200 },
        );
        let pids = vec![10, 11];

        // No rollup anywhere → plain RSS sum.
        assert_eq!(tree_memory_kb(&samples, &pids, 300 * PAGE_SIZE_KB), 300 * PAGE_SIZE_KB);
    }
}
