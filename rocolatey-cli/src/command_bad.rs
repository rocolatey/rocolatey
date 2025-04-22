use rocolatey_lib::roco::local::get_local_bad_packages_text;

pub fn bad(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    print!("{}", get_local_bad_packages_text(r));
}
