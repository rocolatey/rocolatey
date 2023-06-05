use clap_complete::{generate_to, shells::Bash, shells::Fish, shells::Zsh, shells::PowerShell, shells::Elvish};

include!("src/cli.rs");

fn main() {
  let mut app = build_cli();
  let appname = "roco";
  app.set_bin_name(appname);

  let outdir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("completions");
  println!("cargo:warning=> completion file directory: {:?}", outdir);

  let path = generate_to::<Bash, _, _>(Bash, &mut app, appname, &outdir);
  println!("cargo:warning=> Bash completion file: {:?}", path);

  let path = generate_to::<Fish, _, _>(Fish, &mut app, appname, &outdir);
  println!("cargo:warning=> Fish completion file: {:?}", path);

  let path = generate_to::<Zsh, _, _>(Zsh, &mut app, appname, &outdir);
  println!("cargo:warning=> Zsh completion file: {:?}", path);

  let path = generate_to::<PowerShell, _, _>(PowerShell, &mut app, appname, &outdir);
  println!("cargo:warning=> PowerShell completion file: {:?}", path);

  let path = generate_to::<Elvish, _, _>(Elvish, &mut app, appname, &outdir);
  println!("cargo:warning=> Elvish completion file: {:?}", path);

}
