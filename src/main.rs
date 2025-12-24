mod brew;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, CheckButton, Label, Orientation,
    ScrolledWindow, ListBox, ListBoxRow, Stack, StackSidebar, SearchEntry,
    Paned, Spinner, TextView, Window,
};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "io.github.brewhouse.app";

fn main() {
    // Set program name before GTK init to control WM_CLASS
    glib::set_prgname(Some("brewhouse"));
    glib::set_application_name("BrewHouse");

    adw::init().expect("Failed to initialize libadwaita");

    if !brew::is_brew_installed() {
        eprintln!("Homebrew is not installed!");
    }

    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_startup(|_| {
        load_css();
    });

    app.connect_activate(|app| {
        build_ui(app);
    });

    app.run();
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        r#"
        * {
            font-size: 14px;
        }
        .title-1 {
            font-size: 24px;
            font-weight: bold;
        }
        .title-2 {
            font-size: 20px;
            font-weight: bold;
        }
        .title-3 {
            font-size: 18px;
            font-weight: bold;
        }
        .heading {
            font-size: 16px;
            font-weight: 600;
        }
        .caption {
            font-size: 13px;
        }
        .dim-label {
            opacity: 0.7;
        }
        .card {
            background: alpha(@card_bg_color, 0.8);
            border-radius: 8px;
            padding: 8px;
        }
        "#,
    );

    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not get default display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_ui(app: &Application) {
    let app_clone = app.clone();

    // Show update dialog first
    show_update_dialog(app, move || {
        build_main_window(&app_clone);
    });
}

fn show_update_dialog<F: Fn() + 'static>(app: &Application, on_complete: F) {
    let dialog = Window::builder()
        .application(app)
        .title("BrewHouse - Updating")
        .default_width(600)
        .default_height(400)
        .modal(true)
        .build();

    dialog.set_icon_name(None);

    let vbox = Box::new(Orientation::Vertical, 10);
    vbox.set_margin_start(20);
    vbox.set_margin_end(20);
    vbox.set_margin_top(20);
    vbox.set_margin_bottom(20);

    // Header
    let header_box = Box::new(Orientation::Horizontal, 10);
    let spinner = Spinner::new();
    spinner.set_spinning(true);
    header_box.append(&spinner);

    let status_label = Label::new(Some("Updating Homebrew..."));
    status_label.add_css_class("title-3");
    header_box.append(&status_label);
    vbox.append(&header_box);

    // Output text view
    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);

    let text_view = TextView::new();
    text_view.set_editable(false);
    text_view.set_wrap_mode(gtk4::WrapMode::Word);
    text_view.set_monospace(true);
    scroll.set_child(Some(&text_view));
    vbox.append(&scroll);

    // Continue button (hidden until complete)
    let continue_btn = Button::with_label("Continue");
    continue_btn.add_css_class("suggested-action");
    continue_btn.set_visible(false);
    continue_btn.set_halign(gtk4::Align::End);
    vbox.append(&continue_btn);

    dialog.set_child(Some(&vbox));
    dialog.present();

    // Run brew update
    let spinner_clone = spinner.clone();
    let status_label_clone = status_label.clone();
    let text_view_clone = text_view.clone();
    let continue_btn_clone = continue_btn.clone();
    let dialog_clone = dialog.clone();

    glib::spawn_future_local(async move {
        let result = gtk4::gio::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(brew::update_brew())
        })
        .await
        .expect("Background task failed");

        spinner_clone.set_spinning(false);
        spinner_clone.set_visible(false);

        let buffer = text_view_clone.buffer();

        match result {
            Ok((stdout, stderr)) => {
                status_label_clone.set_text("Homebrew updated successfully");

                let mut output = String::new();
                if !stderr.is_empty() {
                    output.push_str(&stderr);
                }
                if !stdout.is_empty() {
                    if !output.is_empty() {
                        output.push_str("\n");
                    }
                    output.push_str(&stdout);
                }
                if output.trim().is_empty() {
                    output = "Already up-to-date.".to_string();
                }
                buffer.set_text(&output);
            }
            Err(e) => {
                status_label_clone.set_text("Update failed (continuing anyway)");
                buffer.set_text(&format!("Error: {}", e));
            }
        }

        continue_btn_clone.set_visible(true);
    });

    // Continue button handler
    let dialog_for_btn = dialog.clone();
    continue_btn.connect_clicked(move |_| {
        dialog_for_btn.close();
        on_complete();
    });
}

fn build_main_window(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("BrewHouse")
        .default_width(1200)
        .default_height(700)
        .build();

    // Clear any cached icon
    window.set_icon_name(None);

    let main_box = Box::new(Orientation::Horizontal, 0);

    let stack = Stack::new();
    stack.set_hexpand(true);

    stack.add_titled(&create_installed_view(), Some("installed"), "Installed");
    stack.add_titled(&create_browse_view(), Some("browse"), "Browse");
    stack.add_titled(&create_updates_view(), Some("updates"), "Updates");

    // Left panel: sidebar + stats
    let left_panel = Box::new(Orientation::Vertical, 0);
    left_panel.set_width_request(200);

    let stack_sidebar = StackSidebar::new();
    stack_sidebar.set_stack(&stack);
    stack_sidebar.set_vexpand(true);
    left_panel.append(&stack_sidebar);

    // Stats panel
    let stats_frame = Box::new(Orientation::Vertical, 4);
    stats_frame.set_margin_start(10);
    stats_frame.set_margin_end(10);
    stats_frame.set_margin_top(10);
    stats_frame.set_margin_bottom(10);
    stats_frame.add_css_class("card");

    let stats_header = Label::new(Some("Status"));
    stats_header.add_css_class("heading");
    stats_header.set_halign(gtk4::Align::Start);
    stats_frame.append(&stats_header);

    let stats_grid = gtk4::Grid::new();
    stats_grid.set_row_spacing(4);
    stats_grid.set_column_spacing(8);

    // Create stat labels
    let stat_installed = create_stat_row(&stats_grid, 0, "Installed:", "...");
    let stat_casks = create_stat_row(&stats_grid, 1, "Casks:", "...");
    let stat_outdated = create_stat_row(&stats_grid, 2, "Outdated:", "...");
    let stat_formulae = create_stat_row(&stats_grid, 3, "Formulae:", "...");
    let stat_leaves = create_stat_row(&stats_grid, 4, "Leaves:", "...");
    let stat_taps = create_stat_row(&stats_grid, 5, "Taps:", "...");

    stats_frame.append(&stats_grid);
    left_panel.append(&stats_frame);

    main_box.append(&left_panel);
    main_box.append(&stack);

    window.set_child(Some(&main_box));
    window.present();

    // Load stats asynchronously
    glib::spawn_future_local(async move {
        let result = gtk4::gio::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(brew::get_brew_stats())
        })
        .await
        .expect("Background task failed");

        if let Ok(stats) = result {
            stat_installed.set_text(&stats.installed.to_string());
            stat_casks.set_text(&stats.casks.to_string());
            stat_outdated.set_text(&stats.outdated.to_string());
            stat_formulae.set_text(&stats.formulae.to_string());
            stat_leaves.set_text(&stats.leaves.to_string());
            stat_taps.set_text(&stats.taps.to_string());
        }
    });
}

fn create_stat_row(grid: &gtk4::Grid, row: i32, label: &str, value: &str) -> Label {
    let name_label = Label::new(Some(label));
    name_label.set_halign(gtk4::Align::Start);
    name_label.add_css_class("dim-label");

    let value_label = Label::new(Some(value));
    value_label.set_halign(gtk4::Align::End);
    value_label.set_hexpand(true);

    grid.attach(&name_label, 0, row, 1, 1);
    grid.attach(&value_label, 1, row, 1, 1);

    value_label
}

// ============================================================================
// Installed View
// ============================================================================

fn create_installed_view() -> Box {
    let view = Box::new(Orientation::Vertical, 10);
    view.set_margin_start(10);
    view.set_margin_end(10);
    view.set_margin_top(10);
    view.set_margin_bottom(10);

    // Header with status
    let header_box = Box::new(Orientation::Horizontal, 10);
    let header = Label::new(Some("Installed Packages"));
    header.add_css_class("title-2");
    header_box.append(&header);

    let spinner = Spinner::new();
    spinner.set_spinning(true);
    header_box.append(&spinner);

    let status_label = Label::new(Some("Loading..."));
    status_label.set_hexpand(true);
    status_label.set_halign(gtk4::Align::Start);
    header_box.append(&status_label);

    view.append(&header_box);

    // Split pane: list | details
    let paned = Paned::new(Orientation::Horizontal);
    paned.set_vexpand(true);
    paned.set_position(400);

    // Left: package list
    let list_scroll = ScrolledWindow::new();
    list_scroll.set_vexpand(true);
    let list_box = ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    list_box.add_css_class("boxed-list");
    list_scroll.set_child(Some(&list_box));

    // Right: details panel
    let details_box = Box::new(Orientation::Vertical, 10);
    details_box.set_margin_start(20);
    details_box.set_margin_end(20);
    details_box.set_margin_top(20);
    details_box.set_margin_bottom(20);
    details_box.set_hexpand(true);

    let details_name = Label::new(Some("Select a package"));
    details_name.add_css_class("title-1");
    details_name.set_halign(gtk4::Align::Start);
    details_box.append(&details_name);

    let details_version = Label::new(None);
    details_version.set_halign(gtk4::Align::Start);
    details_version.add_css_class("dim-label");
    details_box.append(&details_version);

    let details_desc = Label::new(None);
    details_desc.set_halign(gtk4::Align::Start);
    details_desc.set_wrap(true);
    details_desc.set_max_width_chars(50);
    details_box.append(&details_desc);

    let details_homepage = Label::new(None);
    details_homepage.set_halign(gtk4::Align::Start);
    details_homepage.set_selectable(true);
    details_homepage.add_css_class("dim-label");
    details_box.append(&details_homepage);

    // Uninstall button (hidden until package selected)
    let uninstall_btn = Button::with_label("Uninstall");
    uninstall_btn.add_css_class("destructive-action");
    uninstall_btn.set_halign(gtk4::Align::Start);
    uninstall_btn.set_margin_top(20);
    uninstall_btn.set_visible(false);
    details_box.append(&uninstall_btn);

    let uninstall_status = Label::new(None);
    uninstall_status.set_halign(gtk4::Align::Start);
    details_box.append(&uninstall_status);

    // Spacer
    let spacer = Box::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    details_box.append(&spacer);

    paned.set_start_child(Some(&list_scroll));
    paned.set_end_child(Some(&details_box));
    view.append(&paned);

    // Store packages for lookup
    let packages_store: Rc<RefCell<Vec<brew::Package>>> = Rc::new(RefCell::new(Vec::new()));

    // Row selection handler
    let packages_for_selection = packages_store.clone();
    let details_name_clone = details_name.clone();
    let details_version_clone = details_version.clone();
    let details_desc_clone = details_desc.clone();
    let details_homepage_clone = details_homepage.clone();
    let uninstall_btn_clone = uninstall_btn.clone();

    list_box.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let idx = row.index() as usize;
            let packages = packages_for_selection.borrow();
            if let Some(pkg) = packages.get(idx) {
                details_name_clone.set_text(&pkg.name);
                details_version_clone.set_text(&format!("Version: {}", pkg.version.as_deref().unwrap_or("unknown")));
                details_desc_clone.set_text(pkg.desc.as_deref().unwrap_or("No description available"));
                if let Some(hp) = &pkg.homepage {
                    details_homepage_clone.set_text(hp);
                    details_homepage_clone.set_visible(true);
                } else {
                    details_homepage_clone.set_visible(false);
                }
                uninstall_btn_clone.set_visible(true);
            }
        }
    });

    // Uninstall button handler
    let packages_for_uninstall = packages_store.clone();
    let list_box_for_uninstall = list_box.clone();
    let uninstall_status_clone = uninstall_status.clone();
    let details_name_for_uninstall = details_name.clone();
    let uninstall_btn_for_handler = uninstall_btn.clone();

    uninstall_btn.connect_clicked(move |btn| {
        let selected_row = list_box_for_uninstall.selected_row();
        if let Some(row) = selected_row {
            let idx = row.index() as usize;
            let packages = packages_for_uninstall.borrow();
            if let Some(pkg) = packages.get(idx) {
                let pkg_name = pkg.name.clone();
                let status_label = uninstall_status_clone.clone();
                let btn_clone = btn.clone();
                let row_clone = row.clone();
                let list_box_clone = list_box_for_uninstall.clone();
                let details_name_clone = details_name_for_uninstall.clone();
                let uninstall_btn_clone = uninstall_btn_for_handler.clone();

                btn.set_sensitive(false);
                status_label.set_text("Uninstalling...");

                glib::spawn_future_local(async move {
                    let result = gtk4::gio::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(brew::uninstall_package(&pkg_name))
                    })
                    .await
                    .expect("Background task failed");

                    match result {
                        Ok(_) => {
                            status_label.set_text("Uninstalled successfully!");
                            list_box_clone.remove(&row_clone);
                            details_name_clone.set_text("Package uninstalled");
                            uninstall_btn_clone.set_visible(false);
                        }
                        Err(e) => {
                            status_label.set_text(&format!("Error: {}", e));
                            btn_clone.set_sensitive(true);
                        }
                    }
                });
            }
        }
    });

    // Load packages async
    let list_box_clone = list_box.clone();
    let spinner_clone = spinner.clone();
    let status_label_clone = status_label.clone();
    let packages_store_clone = packages_store.clone();

    glib::spawn_future_local(async move {
        let result = gtk4::gio::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(brew::get_installed_packages())
        })
        .await
        .expect("Background task failed");

        match result {
            Ok(packages) => {
                spinner_clone.set_spinning(false);
                spinner_clone.set_visible(false);
                status_label_clone.set_text(&format!("{} packages", packages.len()));

                for package in &packages {
                    let row = create_package_row(&package.name, package.version.as_deref(), package.desc.as_deref());
                    list_box_clone.append(&row);
                }
                *packages_store_clone.borrow_mut() = packages;
            }
            Err(e) => {
                spinner_clone.set_spinning(false);
                spinner_clone.set_visible(false);
                status_label_clone.set_text(&format!("Error: {}", e));
            }
        }
    });

    view
}

// ============================================================================
// Browse View
// ============================================================================

fn create_browse_view() -> Box {
    let view = Box::new(Orientation::Vertical, 10);
    view.set_margin_start(10);
    view.set_margin_end(10);
    view.set_margin_top(10);
    view.set_margin_bottom(10);

    // Search bar
    let search_box = Box::new(Orientation::Horizontal, 10);
    let search_entry = SearchEntry::new();
    search_entry.set_placeholder_text(Some("Search packages..."));
    search_entry.set_hexpand(true);
    search_box.append(&search_entry);

    let search_spinner = Spinner::new();
    search_box.append(&search_spinner);

    let search_status = Label::new(Some("Enter a search term"));
    search_status.add_css_class("dim-label");
    search_box.append(&search_status);

    view.append(&search_box);

    // Split pane
    let paned = Paned::new(Orientation::Horizontal);
    paned.set_vexpand(true);
    paned.set_position(400);

    // Left: results list
    let list_scroll = ScrolledWindow::new();
    list_scroll.set_vexpand(true);
    let list_box = ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    list_box.add_css_class("boxed-list");
    list_scroll.set_child(Some(&list_box));

    // Right: details
    let details_box = Box::new(Orientation::Vertical, 10);
    details_box.set_margin_start(20);
    details_box.set_margin_end(20);
    details_box.set_margin_top(20);

    let details_name = Label::new(Some("Select a package"));
    details_name.add_css_class("title-1");
    details_name.set_halign(gtk4::Align::Start);
    details_box.append(&details_name);

    let details_version = Label::new(None);
    details_version.set_halign(gtk4::Align::Start);
    details_version.add_css_class("dim-label");
    details_box.append(&details_version);

    let details_desc = Label::new(None);
    details_desc.set_halign(gtk4::Align::Start);
    details_desc.set_wrap(true);
    details_desc.set_max_width_chars(50);
    details_box.append(&details_desc);

    let details_homepage = Label::new(None);
    details_homepage.set_halign(gtk4::Align::Start);
    details_homepage.set_selectable(true);
    details_homepage.add_css_class("dim-label");
    details_box.append(&details_homepage);

    // Install button
    let install_btn = Button::with_label("Install");
    install_btn.add_css_class("suggested-action");
    install_btn.set_halign(gtk4::Align::Start);
    install_btn.set_margin_top(20);
    install_btn.set_visible(false);
    details_box.append(&install_btn);

    let install_status = Label::new(None);
    install_status.set_halign(gtk4::Align::Start);
    details_box.append(&install_status);

    paned.set_start_child(Some(&list_scroll));
    paned.set_end_child(Some(&details_box));
    view.append(&paned);

    // Store search results
    let results_store: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    // Search handler
    let list_box_for_search = list_box.clone();
    let search_spinner_clone = search_spinner.clone();
    let search_status_clone = search_status.clone();
    let results_store_clone = results_store.clone();
    let details_name_reset = details_name.clone();
    let details_version_reset = details_version.clone();
    let details_desc_reset = details_desc.clone();
    let install_btn_reset = install_btn.clone();

    search_entry.connect_activate(move |entry| {
        let query = entry.text().to_string();
        eprintln!("Search activated with query: '{}'", query);
        if query.is_empty() {
            eprintln!("Empty query, returning");
            return;
        }

        // Clear previous results
        while let Some(child) = list_box_for_search.first_child() {
            list_box_for_search.remove(&child);
        }
        details_name_reset.set_text("Searching...");
        details_version_reset.set_text("");
        details_desc_reset.set_text("");
        install_btn_reset.set_visible(false);

        search_spinner_clone.set_spinning(true);
        search_status_clone.set_text("Searching...");

        let list_box_clone = list_box_for_search.clone();
        let spinner_clone = search_spinner_clone.clone();
        let status_clone = search_status_clone.clone();
        let results_clone = results_store_clone.clone();
        let details_name_clone = details_name_reset.clone();

        eprintln!("Spawning search task...");
        glib::spawn_future_local(async move {
            eprintln!("Search task started for query");
            let result = gtk4::gio::spawn_blocking(move || {
                eprintln!("Running brew search...");
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(brew::search_packages(&query))
            })
            .await
            .expect("Background task failed");

            eprintln!("Search completed: {:?}", result.as_ref().map(|v| v.len()));
            spinner_clone.set_spinning(false);

            match result {
                Ok(packages) => {
                    eprintln!("Found {} packages", packages.len());
                    status_clone.set_text(&format!("{} results", packages.len()));
                    details_name_clone.set_text("Select a package");

                    for pkg_name in &packages {
                        let row = create_simple_row(pkg_name);
                        list_box_clone.append(&row);
                    }
                    *results_clone.borrow_mut() = packages;
                }
                Err(e) => {
                    status_clone.set_text(&format!("Error: {}", e));
                }
            }
        });
    });

    // Row selection - fetch package info
    let results_for_selection = results_store.clone();
    let details_name_clone = details_name.clone();
    let details_version_clone = details_version.clone();
    let details_desc_clone = details_desc.clone();
    let details_homepage_clone = details_homepage.clone();
    let install_btn_clone = install_btn.clone();
    let install_status_clone = install_status.clone();

    list_box.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let idx = row.index() as usize;
            let results = results_for_selection.borrow();
            if let Some(pkg_name) = results.get(idx) {
                let pkg_name = pkg_name.clone();
                let name_label = details_name_clone.clone();
                let version_label = details_version_clone.clone();
                let desc_label = details_desc_clone.clone();
                let homepage_label = details_homepage_clone.clone();
                let btn = install_btn_clone.clone();
                let status = install_status_clone.clone();

                name_label.set_text("Loading...");
                version_label.set_text("");
                desc_label.set_text("");
                homepage_label.set_text("");
                btn.set_visible(false);
                status.set_text("");

                glib::spawn_future_local(async move {
                    let result = gtk4::gio::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(brew::get_package_info(&pkg_name))
                    })
                    .await
                    .expect("Background task failed");

                    match result {
                        Ok(info) => {
                            name_label.set_text(&info.name);
                            version_label.set_text(&format!("Version: {}", info.versions.stable));
                            desc_label.set_text(info.desc.as_deref().unwrap_or("No description"));
                            if let Some(hp) = &info.homepage {
                                homepage_label.set_text(hp);
                                homepage_label.set_visible(true);
                            } else {
                                homepage_label.set_visible(false);
                            }
                            btn.set_visible(true);
                        }
                        Err(e) => {
                            name_label.set_text("Error loading package");
                            desc_label.set_text(&e.to_string());
                        }
                    }
                });
            }
        }
    });

    // Install button handler
    let results_for_install = results_store.clone();
    let list_box_for_install = list_box.clone();
    let install_status_for_handler = install_status.clone();

    install_btn.connect_clicked(move |btn| {
        let selected_row = list_box_for_install.selected_row();
        if let Some(row) = selected_row {
            let idx = row.index() as usize;
            let results = results_for_install.borrow();
            if let Some(pkg_name) = results.get(idx) {
                let pkg_name = pkg_name.clone();
                let status = install_status_for_handler.clone();
                let btn_clone = btn.clone();

                btn.set_sensitive(false);
                status.set_text("Installing...");

                glib::spawn_future_local(async move {
                    let result = gtk4::gio::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(brew::install_package(&pkg_name))
                    })
                    .await
                    .expect("Background task failed");

                    match result {
                        Ok(_) => {
                            status.set_text("Installed successfully!");
                        }
                        Err(e) => {
                            status.set_text(&format!("Error: {}", e));
                            btn_clone.set_sensitive(true);
                        }
                    }
                });
            }
        }
    });

    view
}

// ============================================================================
// Updates View
// ============================================================================

fn create_updates_view() -> Box {
    let view = Box::new(Orientation::Vertical, 10);
    view.set_margin_start(10);
    view.set_margin_end(10);
    view.set_margin_top(10);
    view.set_margin_bottom(10);

    // Header
    let header_box = Box::new(Orientation::Horizontal, 10);
    let header = Label::new(Some("Available Updates"));
    header.add_css_class("title-2");
    header_box.append(&header);

    let spinner = Spinner::new();
    spinner.set_spinning(true);
    header_box.append(&spinner);

    let status_label = Label::new(Some("Checking for updates..."));
    status_label.set_hexpand(true);
    status_label.set_halign(gtk4::Align::Start);
    header_box.append(&status_label);

    // Upgrade Selected button
    let upgrade_selected_btn = Button::with_label("Upgrade Selected");
    upgrade_selected_btn.add_css_class("suggested-action");
    upgrade_selected_btn.set_visible(false);
    header_box.append(&upgrade_selected_btn);

    // Upgrade All button
    let upgrade_all_btn = Button::with_label("Upgrade All");
    upgrade_all_btn.set_visible(false);
    header_box.append(&upgrade_all_btn);

    view.append(&header_box);

    // List of outdated packages
    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    let list_box = ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::None);
    list_box.add_css_class("boxed-list");
    scroll.set_child(Some(&list_box));
    view.append(&scroll);

    // Upgrade status
    let upgrade_status = Label::new(None);
    upgrade_status.set_halign(gtk4::Align::Start);
    view.append(&upgrade_status);

    // Store checkboxes for access
    let checkboxes: Rc<RefCell<Vec<(String, CheckButton)>>> = Rc::new(RefCell::new(Vec::new()));

    // Load outdated packages
    let list_box_clone = list_box.clone();
    let spinner_clone = spinner.clone();
    let status_label_clone = status_label.clone();
    let upgrade_all_btn_clone = upgrade_all_btn.clone();
    let upgrade_selected_btn_clone = upgrade_selected_btn.clone();
    let checkboxes_clone = checkboxes.clone();

    glib::spawn_future_local(async move {
        let result = gtk4::gio::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(brew::get_outdated_packages())
        })
        .await
        .expect("Background task failed");

        spinner_clone.set_spinning(false);
        spinner_clone.set_visible(false);

        match result {
            Ok(packages) => {
                if packages.is_empty() {
                    status_label_clone.set_text("All packages are up to date!");
                } else {
                    status_label_clone.set_text(&format!("{} updates available", packages.len()));
                    upgrade_all_btn_clone.set_visible(true);
                    upgrade_selected_btn_clone.set_visible(true);

                    let mut cbs = checkboxes_clone.borrow_mut();
                    for pkg_name in packages {
                        let (row, checkbox) = create_update_row_with_checkbox(&pkg_name);
                        cbs.push((pkg_name, checkbox));
                        list_box_clone.append(&row);
                    }
                }
            }
            Err(e) => {
                status_label_clone.set_text(&format!("Error: {}", e));
            }
        }
    });

    // Upgrade Selected handler
    let checkboxes_for_selected = checkboxes.clone();
    let upgrade_status_selected = upgrade_status.clone();
    let list_box_for_selected = list_box.clone();
    let status_for_selected = status_label.clone();
    let upgrade_all_for_selected = upgrade_all_btn.clone();
    let upgrade_selected_for_handler = upgrade_selected_btn.clone();

    upgrade_selected_btn.connect_clicked(move |btn| {
        // Debug: show all checkbox states
        {
            let cbs = checkboxes_for_selected.borrow();
            eprintln!("Checkbox states ({} total):", cbs.len());
            for (name, cb) in cbs.iter() {
                eprintln!("  {} = {}", name, cb.is_active());
            }
        }

        let selected: Vec<String> = checkboxes_for_selected
            .borrow()
            .iter()
            .filter(|(_, cb)| cb.is_active())
            .map(|(name, _)| name.clone())
            .collect();

        eprintln!("Selected for upgrade: {:?}", selected);

        if selected.is_empty() {
            upgrade_status_selected.set_text("No packages selected");
            return;
        }

        btn.set_sensitive(false);
        let total = selected.len();

        let status = upgrade_status_selected.clone();
        let list_box = list_box_for_selected.clone();
        let header_status = status_for_selected.clone();
        let btn_clone = btn.clone();
        let checkboxes_clone = checkboxes_for_selected.clone();
        let upgrade_all_clone = upgrade_all_for_selected.clone();
        let upgrade_selected_clone = upgrade_selected_for_handler.clone();

        // Upgrade packages one by one with progress updates
        glib::spawn_future_local(async move {
            let mut succeeded = Vec::new();
            let mut failed: Vec<(String, String)> = Vec::new();

            for (i, pkg) in selected.iter().enumerate() {
                status.set_text(&format!("Upgrading {} ({}/{})...", pkg, i + 1, total));

                let pkg_clone = pkg.clone();
                let result = gtk4::gio::spawn_blocking(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(brew::upgrade_packages(Some(&pkg_clone)))
                })
                .await
                .expect("Background task failed");

                match result {
                    Ok(_) => succeeded.push(pkg.clone()),
                    Err(e) => failed.push((pkg.clone(), e.to_string())),
                }
            }

            // Clear the list UI
            while let Some(child) = list_box.first_child() {
                list_box.remove(&child);
            }

            // Get remaining packages (failed ones + ones not attempted)
            let remaining: Vec<String> = {
                let cbs = checkboxes_clone.borrow();
                cbs.iter()
                    .filter(|(name, _)| !succeeded.contains(name))
                    .map(|(name, _)| name.clone())
                    .collect()
            };

            // Show results
            if failed.is_empty() {
                status.set_text(&format!("{} packages upgraded successfully!", succeeded.len()));
            } else {
                let failed_names: Vec<&str> = failed.iter().map(|(n, _)| n.as_str()).collect();
                let error_msg = failed.iter().map(|(n, e)| format!("{}: {}", n, e)).collect::<Vec<_>>().join("\n");
                status.set_text(&format!(
                    "{} upgraded, {} failed: {}",
                    succeeded.len(),
                    failed.len(),
                    failed_names.join(", ")
                ));
                eprintln!("Upgrade errors:\n{}", error_msg);
            }

            // Rebuild checkboxes store and UI
            {
                let mut cbs = checkboxes_clone.borrow_mut();
                cbs.clear();

                if remaining.is_empty() {
                    header_status.set_text("All packages are up to date!");
                    upgrade_all_clone.set_visible(false);
                    upgrade_selected_clone.set_visible(false);
                } else {
                    header_status.set_text(&format!("{} updates available", remaining.len()));
                    for name in remaining {
                        let (row, new_cb) = create_update_row_with_checkbox(&name);
                        list_box.append(&row);
                        cbs.push((name, new_cb));
                    }
                }
            }
            btn_clone.set_sensitive(true);
        });
    });

    // Upgrade All handler
    let upgrade_status_clone = upgrade_status.clone();
    let list_box_for_upgrade = list_box.clone();
    let status_for_upgrade = status_label.clone();
    let upgrade_selected_for_all = upgrade_selected_btn.clone();

    upgrade_all_btn.connect_clicked(move |btn| {
        btn.set_sensitive(false);
        upgrade_status_clone.set_text("Upgrading all packages...");

        let status = upgrade_status_clone.clone();
        let list_box = list_box_for_upgrade.clone();
        let header_status = status_for_upgrade.clone();
        let btn_clone = btn.clone();
        let upgrade_selected_clone = upgrade_selected_for_all.clone();

        glib::spawn_future_local(async move {
            let result = gtk4::gio::spawn_blocking(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(brew::upgrade_packages(None))
            })
            .await
            .expect("Background task failed");

            match result {
                Ok(_) => {
                    status.set_text("All packages upgraded successfully!");
                    while let Some(child) = list_box.first_child() {
                        list_box.remove(&child);
                    }
                    header_status.set_text("All packages are up to date!");
                    btn_clone.set_visible(false);
                    upgrade_selected_clone.set_visible(false);
                }
                Err(e) => {
                    status.set_text(&format!("Error: {}", e));
                    btn_clone.set_sensitive(true);
                }
            }
        });
    });

    view
}

// ============================================================================
// Helper functions
// ============================================================================

fn create_package_row(name: &str, version: Option<&str>, description: Option<&str>) -> ListBoxRow {
    let row = ListBoxRow::new();

    let hbox = Box::new(Orientation::Horizontal, 12);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);

    let info_box = Box::new(Orientation::Vertical, 2);
    info_box.set_hexpand(true);

    let name_label = Label::new(Some(name));
    name_label.set_halign(gtk4::Align::Start);
    name_label.add_css_class("heading");
    info_box.append(&name_label);

    if let Some(ver) = version {
        let version_label = Label::new(Some(ver));
        version_label.set_halign(gtk4::Align::Start);
        version_label.add_css_class("dim-label");
        version_label.add_css_class("caption");
        info_box.append(&version_label);
    }

    if let Some(desc) = description {
        let desc_label = Label::new(Some(desc));
        desc_label.set_halign(gtk4::Align::Start);
        desc_label.set_wrap(true);
        desc_label.set_max_width_chars(50);
        desc_label.add_css_class("caption");
        info_box.append(&desc_label);
    }

    hbox.append(&info_box);
    row.set_child(Some(&hbox));
    row
}

fn create_simple_row(name: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    let label = Label::new(Some(name));
    label.set_halign(gtk4::Align::Start);
    label.set_margin_start(12);
    label.set_margin_end(12);
    label.set_margin_top(8);
    label.set_margin_bottom(8);
    row.set_child(Some(&label));
    row
}

fn create_update_row_with_checkbox(name: &str) -> (ListBoxRow, CheckButton) {
    let row = ListBoxRow::new();

    let hbox = Box::new(Orientation::Horizontal, 12);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);

    let checkbox = CheckButton::new();
    hbox.append(&checkbox);

    let label = Label::new(Some(name));
    label.set_halign(gtk4::Align::Start);
    label.set_hexpand(true);
    label.add_css_class("heading");
    hbox.append(&label);

    let update_icon = Label::new(Some("⬆"));
    update_icon.add_css_class("dim-label");
    hbox.append(&update_icon);

    row.set_child(Some(&hbox));
    (row, checkbox)
}
