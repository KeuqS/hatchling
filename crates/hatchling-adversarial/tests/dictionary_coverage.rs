//! The probe's `LIB_CANDIDATES` dictionary must cover every shared
//! library the probe binary actually loads: the in-cell mount and
//! setuid sweeps stat exactly the dictionary entries, so a toolchain
//! change that adds a library would otherwise silently narrow the
//! sweep. This test runs `ldd` on the compiled probe binary and fails
//! loudly on any library the dictionary misses.

use hatchling_adversarial::dictionaries::LIB_CANDIDATES;

/// The loader-visible path on each `ldd` line: the path after `=>`,
/// or the leading absolute path for the interpreter line.
fn ldd_targets(output: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.starts_with("linux-vdso") || line.starts_with("linux-gate") {
            continue;
        }
        if let Some((_, after)) = line.split_once("=>") {
            let path = after.split_whitespace().next().unwrap_or("");
            if path.starts_with('/') {
                targets.push(path.to_owned());
            }
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        if first.starts_with('/') {
            targets.push(first.to_owned());
        }
    }
    targets.sort();
    targets.dedup();
    targets
}

#[test]
fn lib_dictionary_covers_probe_libraries() -> Result<(), String> {
    if cfg!(not(target_arch = "x86_64")) {
        use std::io::Write as _;
        let _ = writeln!(
            std::io::stdout(),
            "SKIP: dictionary coverage is x86_64-only"
        );
        return Ok(());
    }
    let probe = env!("CARGO_BIN_EXE_hatchling-probe");
    let output = std::process::Command::new("ldd")
        .arg(probe)
        .output()
        .map_err(|e| format!("cannot run ldd on {probe}: {e} — install libc-bin"))?;
    if !output.status.success() {
        return Err(format!(
            "ldd failed on {probe}: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("not a dynamic executable") {
        // A statically linked probe needs no library dictionary.
        return Ok(());
    }
    let targets = ldd_targets(&stdout);
    let missing: Vec<&str> = targets
        .iter()
        .filter(|t| !LIB_CANDIDATES.contains(&t.as_str()))
        .map(String::as_str)
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "LIB_CANDIDATES misses libraries the probe loads: {missing:?}. \
         Add them to crates/hatchling-adversarial/src/dictionaries.rs"
    ))
}

#[test]
fn ldd_parser_extracts_loader_paths() {
    let sample = "\
\tlinux-vdso.so.1 (0x00007ffc12345000)\n\
\tlibm.so.6 => /lib/x86_64-linux-gnu/libm.so.6 (0x00007f1234500000)\n\
\tlibc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007f1234400000)\n\
\t/lib64/ld-linux-x86-64.so.2 (0x00007f1234600000)\n";
    assert_eq!(
        ldd_targets(sample),
        vec![
            "/lib/x86_64-linux-gnu/libc.so.6",
            "/lib/x86_64-linux-gnu/libm.so.6",
            "/lib64/ld-linux-x86-64.so.2",
        ]
    );
    assert!(ldd_targets("").is_empty());
}
