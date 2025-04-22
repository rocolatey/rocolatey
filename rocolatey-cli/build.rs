use std::{env, fs, path::PathBuf};

use clap_complete::{
    generate_to, shells::Bash, shells::Elvish, shells::Fish, shells::PowerShell, shells::Zsh,
};

include!("src/cli.rs");

fn cli_completions() {
    let mut app = build_cli();
    let appname = "roco";
    app.set_bin_name(appname);

    let outdir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("completions");
    println!("cargo:info=> completion file directory: {:?}", outdir);

    let path = generate_to::<Bash, _, _>(Bash, &mut app, appname, &outdir);
    println!("cargo:info=> Bash completion file: {:?}", path);

    let path = generate_to::<Fish, _, _>(Fish, &mut app, appname, &outdir);
    println!("cargo:info=> Fish completion file: {:?}", path);

    let path = generate_to::<Zsh, _, _>(Zsh, &mut app, appname, &outdir);
    println!("cargo:info=> Zsh completion file: {:?}", path);

    let path = generate_to::<PowerShell, _, _>(PowerShell, &mut app, appname, &outdir);
    println!("cargo:info=> PowerShell completion file: {:?}", path);

    let path = generate_to::<Elvish, _, _>(Elvish, &mut app, appname, &outdir);
    println!("cargo:info=> Elvish completion file: {:?}", path);
}

fn main() {
    cli_completions();

    let json_path = PathBuf::from("../THIRDPARTY.json");
    let json_content = fs::read_to_string(&json_path).expect("Failed to read THIRDPARTY.json");

    let license_path = PathBuf::from("../LICENSE.txt");
    let license_content = fs::read_to_string(&license_path).expect("Failed to read LICENSE.txt");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(out_dir).join("licenses_json.rs");

    fs::write(
        &dest_path,
        format!(
            "pub static JSON_LICENSE_DATA: &str = r#\"{}\"#;\npub static ROCO_LICENSE_JSON: &str = r#\"{}\"#;",
            json_content, license_content
        ),
    )
    .expect("Failed to write licenses_json.rs");
}
