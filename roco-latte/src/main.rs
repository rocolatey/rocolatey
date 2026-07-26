use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Element, Fill, Length, Theme};
use rocolatey_lib::roco::{get_choco_sources, local, remote, Feed, FeedType, Package};

#[derive(Debug, Clone)]
enum Message {
    SelectSource(usize),
    SelectPackage(Package),
    UpdateSearch(String),
    Search(String),
}

#[derive(Default)]
struct State {
    sources: Vec<Feed>,
    selected_source: Option<usize>,
    packages: Option<Vec<Package>>,
    current_package: Option<Package>,
    search_input: String,
}

fn update(state: &mut State, message: Message) {
    match message {
        Message::SelectSource(i) => {
            state.selected_source = Some(i);

            let feed = &state.sources[i];
            let packages = if feed.name == "local" {
                match local::get_local_packages("") {
                    Ok((pkgs, _)) => Some(pkgs),
                    Err(e) => {
                        eprintln!("Failed to get local packages: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            state.packages = packages;
        }
        Message::SelectPackage(pkg) => {
            state.current_package = Some(pkg);
        }
        Message::UpdateSearch(s) => {
            // update search filter
            state.search_input = s;
        }
        Message::Search(filter) => {
            // triggered by Enter
            state.search_input = filter.clone();
            println!("Searching for packages matching: {}", filter);
            let feed = if let Some(i) = state.selected_source {
                &state.sources[i]
            } else {
                return;
            };
            let packages = if state.selected_source == Some(0) {
                match local::get_local_packages(&filter) {
                    Ok((pkgs, _)) => Some(pkgs),
                    Err(e) => {
                        eprintln!("Failed to get local packages: {}", e);
                        None
                    }
                }
            } else {
                let search_terms: Vec<String> = filter
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                let mut feeds = vec![];
                feeds.push(feed.clone());

                // Run the async function to completion synchronously
                let result =
                    remote::find_latest_remote_packages(&search_terms, true, &feeds, false);
                let result = futures::executor::block_on(result);

                match result {
                    Ok(map) => {
                        // consume the HashMap and collect values into Vec<Package>
                        let pkgs: Vec<Package> = map.into_values().collect();
                        Some(pkgs)
                    }
                    Err(e) => {
                        eprintln!("Failed to get remote packages: {}", e);
                        None
                    }
                }
            };
            state.packages = packages;
        }
    }
}

fn view(state: &State) -> Element<'_, Message> {
    // build selectable sources_list
    let mut sources_list = column![];
    for (i, s) in state.sources.iter().enumerate() {
        let label = if state.selected_source == Some(i) {
            format!("[{}]", s.name)
        } else {
            format!(" {} ", s.name)
        };
        // button that sends SelectSource(i) when clicked
        sources_list = sources_list.push(
            button(text(label))
                .width(Length::Fill)
                .on_press(Message::SelectSource(i)),
        );
    }

    let mut packages_list = column![];
    if let Some(ref state_packages) = state.packages {
        for pkg in state_packages.iter() {
            let label = format!("{} - {}", pkg.id, pkg.version);
            packages_list = packages_list.push(
                button(text(label))
                    .width(Length::Fill)
                    .on_press(Message::SelectPackage(pkg.clone())),
            );
        }
    } else {
        packages_list = packages_list.push(text("No packages loaded."));
    }

    let packagedetails = if let Some(ref pkg) = state.current_package {
        column![
            text(format!("Package ID: {}", pkg.id)),
            text(format!("Version: {}", pkg.version)),
        ]
    } else {
        column![text("Select a package to see details.")]
    };

    let left = scrollable(sources_list)
        .height(Length::Fill)
        .width(Length::Fixed(140.0));

    let right = column![
        text_input("Search packages...", &state.search_input)
            .on_input(Message::UpdateSearch)
            .on_submit(Message::Search(state.search_input.clone()))
            .padding(10)
            .width(Length::Fill),
        scrollable(packages_list)
            .height(Length::Fill)
            .width(Length::Fill),
        packagedetails.width(Length::Fill)
    ]
    .spacing(20);

    let layout = row![left, right].spacing(20);

    container(layout)
        .padding(10)
        .center_x(Fill)
        .center_y(Fill)
        .into()
}

fn new() -> State {
    let sources = match get_choco_sources() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to get choco sources: {}", e);
            Vec::new()
        }
    };
    let local_feed = Feed {
        name: "local".into(),
        url: "".into(),
        credential: None,
        proxy: None,
        disabled: false,
        feed_type: FeedType::Unknown,
        service_index: None,
        certificate: None,
        bypass_proxy: false,
        self_service: false,
        admin_only: false,
        priority: 0,
    };

    let mut sources_with_local: Vec<Feed> = vec![local_feed];
    sources_with_local.extend(sources);

    let packages = match local::get_local_packages("") {
        Ok((pkgs, _)) => Some(pkgs),
        Err(e) => {
            eprintln!("Failed to get local packages: {}", e);
            None
        }
    };

    State {
        sources: sources_with_local,
        selected_source: Some(0),
        packages: packages,
        current_package: None,
        search_input: String::new(),
    }
}

fn theme(_: &State) -> Theme {
    Theme::Dracula
}

fn main() -> iced::Result {
    iced::application(new, update, view).theme(theme).run()
}
