use rocolatey_lib::roco::local::get_sources_text;

pub fn source(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    print!("{}", get_sources_text(r));
}
