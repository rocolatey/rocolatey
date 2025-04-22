use rocolatey_lib::roco::local::get_dependency_tree_text;
use rocolatey_lib::roco::local::get_local_packages_text;

pub fn list(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    let filter = matches.get_one::<String>("filter").unwrap();
    if matches.get_flag("deptree") {
        print!("{}", get_dependency_tree_text(filter));
    } else {
        print!("{}", get_local_packages_text(filter, r));
    }
}
