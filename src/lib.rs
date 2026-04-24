/// Find the z3 binary
pub fn find_z3() -> anyhow::Result<std::path::PathBuf> {
    use std::path::PathBuf;
    if let Ok(p) = std::env::var("Z3_BIN") {
        let pb = PathBuf::from(&p);
        if pb.exists() { return Ok(pb); }
    }
    for c in ["build/z3", "build/release/z3", "build/debug/z3"] {
        let p = PathBuf::from(c);
        if p.exists() { return Ok(p); }
    }
    for dir in std::env::var("PATH").unwrap_or_default().split(':') {
        let p = PathBuf::from(dir).join("z3");
        if p.exists() { return Ok(p); }
    }
    anyhow::bail!("z3 not found: install z3, set Z3_BIN, or add to PATH")
}