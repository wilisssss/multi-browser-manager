//! Per-profile resource usage (feature recommendation 2): RAM + CPU of a
//! browser's whole process tree, read straight from `/proc`.
//!
//! Linux-only by design — this is where MBM's orphan detection lives too.
//! Other platforms return an empty list from the command and the UI hides
//! the panel. CPU% is computed as the delta of utime+stime ticks between two
//! samples divided by wall-clock time (kernel CLK_TCK ≈ 100 on every Linux
//! architecture we support).

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
    /// Resident memory of the whole process tree, in KiB.
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

/// Sums cpu ticks + rss over the process tree rooted at `root` (the browser
/// process itself plus every descendant).
fn tree_totals(samples: &HashMap<u32, ProcSample>, root: u32) -> (u64, u64) {
    // children[ppid] = [pids]
    let mut children: HashMap<i32, Vec<u32>> = HashMap::new();
    for (&pid, s) in samples {
        children.entry(s.ppid).or_default().push(pid);
    }
    let mut total_cpu = 0u64;
    let mut total_rss = 0u64;
    let mut queue = vec![root];
    let mut seen = std::collections::HashSet::new();
    while let Some(pid) = queue.pop() {
        if !seen.insert(pid) {
            continue; // pid cycles are impossible in /proc, but stay safe
        }
        if let Some(s) = samples.get(&pid) {
            total_cpu += s.cpu_ticks;
            total_rss += s.rss_pages;
            if let Some(kids) = children.get(&(pid as i32)) {
                queue.extend_from_slice(kids);
            }
        }
    }
    (total_cpu, total_rss * PAGE_SIZE_KB)
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
            let (cpu_ticks, memory_kb) = tree_totals(&samples, pid);
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
        let (cpu, mem_kb) = tree_totals(&samples, 10);
        assert_eq!(cpu, 60);
        assert_eq!(mem_kb, (100 + 200 + 300) * PAGE_SIZE_KB);
    }
}
