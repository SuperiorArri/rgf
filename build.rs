use std::{fs, path::Path};

fn main() {
    let path = Path::new("VERSION");
    println!("cargo:rerun-if-changed=VERSION");

    let raw = fs::read_to_string(path).expect("Failed to read VERSION file");
    let ver = raw.trim();

    let parts: Vec<&str> = ver.split('.').collect();
    if parts.len() != 3 {
        panic!("VERSION must be MAJOR.MINOR.PATCH, got: {ver}");
    }

    let major = parts[0].parse::<u64>().expect("Invalid MAJOR version");
    let minor = parts[1].parse::<u64>().expect("Invalid MINOR version");
    let patch = parts[2].parse::<u64>().expect("Invalid PATCH version");

    println!("cargo:rustc-env=RGF_VERSION={ver}");
    println!("cargo:rustc-env=RGF_VERSION_MAJOR={major}");
    println!("cargo:rustc-env=RGF_VERSION_MINOR={minor}");
    println!("cargo:rustc-env=RGF_VERSION_PATCH={patch}");
}
