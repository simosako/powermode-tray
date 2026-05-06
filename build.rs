use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const ICON_BALANCED_ID: u16 = 101;
const ICON_PERFORMANCE_ID: u16 = 102;
const ICON_EFFICIENCY_ID: u16 = 103;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/balance.ico");
    println!("cargo:rerun-if-changed=assets/performance.ico");
    println!("cargo:rerun-if-changed=assets/eco.ico");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_DESCRIPTION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_LICENSE");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));

    let pkg_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION missing");
    let pkg_name = env::var("CARGO_PKG_NAME").expect("CARGO_PKG_NAME missing");
    let pkg_description = env::var("CARGO_PKG_DESCRIPTION").unwrap_or_default();
    let version_quad = version_quad(&pkg_version);
    let version_string = format!("{}.0", version_core(&pkg_version));
    let copyright = "Copyright (c) 2026 Akira Shimosako. Licensed under MIT OR Apache-2.0.";

    let rc_path = out_dir.join("powermode-tray.resources.rc");
    let rc_contents = format!(
        concat!(
            "#define IDI_POWERMODE_BALANCED {balanced_id}\n",
            "#define IDI_POWERMODE_PERFORMANCE {performance_id}\n",
            "#define IDI_POWERMODE_EFFICIENCY {efficiency_id}\n",
            "\n",
            "IDI_POWERMODE_BALANCED ICON \"{balanced_icon}\"\n",
            "IDI_POWERMODE_PERFORMANCE ICON \"{performance_icon}\"\n",
            "IDI_POWERMODE_EFFICIENCY ICON \"{efficiency_icon}\"\n",
            "\n",
            "1 VERSIONINFO\n",
            " FILEVERSION {version_quad}\n",
            " PRODUCTVERSION {version_quad}\n",
            " FILEFLAGSMASK 0x3fL\n",
            "#ifdef _DEBUG\n",
            " FILEFLAGS 0x1L\n",
            "#else\n",
            " FILEFLAGS 0x0L\n",
            "#endif\n",
            " FILEOS 0x40004L\n",
            " FILETYPE 0x1L\n",
            " FILESUBTYPE 0x0L\n",
            "BEGIN\n",
            "    BLOCK \"StringFileInfo\"\n",
            "    BEGIN\n",
            "        BLOCK \"040904E4\"\n",
            "        BEGIN\n",
            "            VALUE \"FileDescription\", \"{file_description}\"\n",
            "            VALUE \"FileVersion\", \"{version_string}\"\n",
            "            VALUE \"InternalName\", \"{internal_name}\"\n",
            "            VALUE \"LegalCopyright\", \"{copyright}\"\n",
            "            VALUE \"OriginalFilename\", \"{original_filename}\"\n",
            "            VALUE \"ProductName\", \"{product_name}\"\n",
            "            VALUE \"ProductVersion\", \"{version_string}\"\n",
            "        END\n",
            "    END\n",
            "\n",
            "    BLOCK \"VarFileInfo\"\n",
            "    BEGIN\n",
            "        VALUE \"Translation\", 0x0409, 1252\n",
            "    END\n",
            "END\n"
        ),
        balanced_id = ICON_BALANCED_ID,
        performance_id = ICON_PERFORMANCE_ID,
        efficiency_id = ICON_EFFICIENCY_ID,
        balanced_icon = resource_path(&manifest_dir, "assets/balance.ico"),
        performance_icon = resource_path(&manifest_dir, "assets/performance.ico"),
        efficiency_icon = resource_path(&manifest_dir, "assets/eco.ico"),
        version_quad = version_quad,
        file_description = rc_string(&pkg_description),
        version_string = rc_string(&version_string),
        internal_name = rc_string("powermode-tray.exe"),
        copyright = rc_string(&copyright),
        original_filename = rc_string("powermode-tray.exe"),
        product_name = rc_string(&pkg_name),
    );

    fs::write(&rc_path, rc_contents).expect("failed to write generated resource script");
    let _ = embed_resource::compile(rc_path.to_str().expect("resource path is not valid UTF-8"), embed_resource::NONE);
}

fn resource_path(manifest_dir: &Path, relative_path: &str) -> String {
    manifest_dir.join(relative_path).to_string_lossy().replace('\\', "/")
}

fn rc_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn version_core(version: &str) -> &str {
    version
        .split_once(['-', '+'])
        .map_or(version, |(core, _)| core)
}

fn version_quad(version: &str) -> String {
    let mut parts = [0_u16; 4];

    for (index, part) in version_core(version).split('.').take(3).enumerate() {
        parts[index] = part.parse().expect("package version must be numeric semver");
    }

    format!("{},{},{},{}", parts[0], parts[1], parts[2], parts[3])
}
