fn main() {
    println!("cargo:rerun-if-env-changed=TFA_BUILD_COMMIT");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "HEAD"])
        .output()
    {
        let path = String::from_utf8_lossy(&output.stdout);
        println!("cargo:rerun-if-changed={}", path.trim());
    }
    if let Ok(output) = std::process::Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
    {
        let reference = String::from_utf8_lossy(&output.stdout);
        if !reference.trim().is_empty() {
            println!("cargo:rerun-if-changed=.git/{}", reference.trim());
        }
    }
    let commit = std::env::var("TFA_BUILD_COMMIT")
        .ok()
        .or_else(|| {
            let output = std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".into());
    let dirty = std::env::var_os("TFA_BUILD_COMMIT").is_none()
        && std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .is_ok_and(|o| o.status.success() && !o.stdout.is_empty());
    println!(
        "cargo:rustc-env=TFA_GIT_COMMIT={commit}{}",
        if dirty { "-dirty" } else { "" }
    );
}
