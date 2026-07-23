use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/main.rs");
    println!("cargo:rerun-if-changed=src/rendered_image_item.rs");
    println!("cargo:rerun-if-changed=src/live_camera_item.rs");

    let qt_include_path =
        std::env::var("DEP_QT_INCLUDE_PATH").expect("DEP_QT_INCLUDE_PATH missing");
    let mut config = cpp_build::Config::new();
    if let Ok(flags) = std::env::var("DEP_QT_COMPILE_FLAGS") {
        for flag in flags.split_terminator(';') {
            config.flag(flag);
        }
    }
    // Build from the crate root so cpp! blocks in every module (e.g.
    // rendered_image_item.rs and live_camera_item.rs) are compiled.
    config.include(&qt_include_path).build("src/main.rs");

    build_live_filter(&qt_include_path);
}

// The QtMultimedia video filter needs Q_OBJECT, so it can't live in a `cpp!`
// block (the `cpp` crate never runs moc). Build it as a standalone TU: moc the
// header, then compile it + the generated moc output with the same Qt flags.
fn build_live_filter(qt_include_path: &str) {
    println!("cargo:rerun-if-changed=cpp/live_filter.cpp");
    println!("cargo:rerun-if-changed=cpp/live_filter.h");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR missing"));
    let moc = find_moc();
    let moc_out = out_dir.join("moc_live_filter.cpp");
    let moc_status = Command::new(&moc)
        .arg("-I")
        .arg(qt_include_path)
        .arg("cpp/live_filter.h")
        .arg("-o")
        .arg(&moc_out)
        .status()
        .expect("failed to launch moc");
    if !moc_status.success() {
        panic!("moc failed on cpp/live_filter.h (moc={moc})");
    }

    let mut build = cc::Build::new();
    build.cpp(true);
    if let Ok(flags) = std::env::var("DEP_QT_COMPILE_FLAGS") {
        for flag in flags.split_terminator(';') {
            build.flag(flag);
        }
    }
    build
        .flag_if_supported("-std=c++17")
        .include(qt_include_path)
        .include(format!("{qt_include_path}/QtCore"))
        .include(format!("{qt_include_path}/QtMultimedia"))
        .include(format!("{qt_include_path}/QtQml"))
        .include("cpp")
        .file("cpp/live_filter.cpp")
        .file(&moc_out)
        .compile("live_filter");

    println!("cargo:rustc-link-lib=Qt5Multimedia");
    if let Ok(lib_path) = std::env::var("QT_LIBRARY_PATH") {
        println!("cargo:rustc-link-search=native={lib_path}");
    }
}

fn find_moc() -> String {
    if let Ok(moc) = std::env::var("QT_MOC") {
        return moc;
    }
    for candidate in ["/usr/lib/qt5/bin/moc", "/usr/bin/moc"] {
        if std::path::Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }
    "moc".to_string()
}
