use crate::{
    Event, Job,
    config::{self, Config, Theme},
    ipc::{Monitor, Target},
    layout::{self, Layout},
};
use gtk::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{cell::RefCell, rc::Rc};

type State = Rc<RefCell<Ui>>;

pub struct Ui {
    pub app: gtk::Application,
    pub config: Config,
    pub events: async_channel::Sender<Event>,
    pub jobs: Option<std::sync::mpsc::Sender<Job>>,
    pub monitors: Vec<Monitor>,
    pub picker: Option<gtk::ApplicationWindow>,
    settings: Option<gtk::ApplicationWindow>,
    error_window: Option<gtk::ApplicationWindow>,
    target: Option<Target>,
    cards: Vec<(Layout, gtk::Button)>,
    pub status: Option<gtk::Label>,
    context: Option<gtk::Label>,
    system_dark: bool,
    system_theme: Option<glib::GString>,
}

impl Ui {
    pub fn new(
        app: gtk::Application,
        config: Config,
        events: async_channel::Sender<Event>,
    ) -> Self {
        let provider = gtk::CssProvider::new();
        provider.load_from_data(include_str!("../data/style.css"));
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let system_dark =
            gtk::Settings::default().is_some_and(|s| s.is_gtk_application_prefer_dark_theme());
        let system_theme = gtk::Settings::default().and_then(|s| s.gtk_theme_name());
        let ui = Self {
            app,
            config,
            events,
            jobs: None,
            monitors: vec![],
            picker: None,
            settings: None,
            error_window: None,
            target: None,
            cards: vec![],
            status: None,
            context: None,
            system_dark,
            system_theme,
        };
        ui.theme();
        ui
    }
    fn theme(&self) {
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_theme_name(if self.config.theme == Theme::System {
                self.system_theme.as_deref()
            } else {
                Some("Adwaita")
            });
            settings.set_gtk_application_prefer_dark_theme(match self.config.theme {
                Theme::System => self.system_dark,
                Theme::Light => false,
                Theme::Dark => true,
            });
        }
    }
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.);
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}
fn vertical(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}
fn row(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}
fn save(state: &State) {
    let result = state.borrow().config.save();
    if let Err(error) = result {
        show_error(state, &format!("Could not save settings: {error:#}"));
    }
}
fn layer_window(app: &gtk::Application, monitor_name: Option<&str>) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Mango Layout Tray")
        .decorated(false)
        .build();
    window.add_css_class("mango-overlay");
    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_namespace(Some("mango-layout-tray"));
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);
        window.set_exclusive_zone(-1);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            window.set_anchor(edge, true);
        }
        if let Some(display) = gdk::Display::default() {
            let monitors = display.monitors();
            for i in 0..monitors.n_items() {
                if let Some(monitor) = monitors.item(i).and_downcast::<gdk::Monitor>()
                    && monitor.connector().as_deref() == monitor_name
                {
                    window.set_monitor(Some(&monitor));
                    break;
                }
            }
        }
    } else {
        window.set_default_size(680, 720);
    }
    window
}

pub fn show_picker(state: &State, target: Target) {
    let (app, config, events) = {
        let ui = state.borrow();
        (ui.app.clone(), ui.config.clone(), ui.events.clone())
    };
    if let Some(old) = state.borrow_mut().picker.take() {
        old.close();
    }
    let window = layer_window(&app, Some(&target.monitor));
    let geometry = gdk::Display::default().and_then(|d| {
        let list = d.monitors();
        (0..list.n_items())
            .filter_map(|i| list.item(i).and_downcast::<gdk::Monitor>())
            .find(|m| m.connector().as_deref() == Some(target.monitor.as_str()))
            .map(|m| m.geometry())
    });
    let narrow = geometry.is_some_and(|g| g.width() < 720);
    let columns = if config.drawer || narrow { 2 } else { 4 };
    let viewport_height = geometry
        .map(|g| (g.height() - 260).clamp(180, 600))
        .unwrap_or(520);
    let overlay = gtk::Overlay::new();
    let backdrop = gtk::Button::new();
    backdrop.add_css_class("backdrop");
    backdrop.set_can_focus(false);
    let close = window.downgrade();
    backdrop.connect_clicked(move |_| {
        if let Some(window) = close.upgrade() {
            window.hide();
        }
    });
    overlay.set_child(Some(&backdrop));
    let panel = vertical(0);
    panel.add_css_class("picker");
    panel.set_halign(gtk::Align::End);
    panel.set_valign(if config.drawer {
        gtk::Align::Fill
    } else {
        gtk::Align::Start
    });
    panel.set_margin_top(if config.drawer { 12 } else { 24 });
    panel.set_margin_bottom(12);
    panel.set_margin_end(16);
    panel.set_size_request(if config.drawer || narrow { 410 } else { 664 }, -1);
    let header = row(12);
    header.add_css_class("header");
    let heading = vertical(4);
    heading.set_hexpand(true);
    heading.append(&label("Find your flow.", "title"));
    let context = label("", "dim-label");
    heading.append(&context);
    header.append(&heading);
    let settings = gtk::Button::with_label("Settings");
    if gdk::Display::default().is_some_and(|display| {
        gtk::IconTheme::for_display(&display).has_icon("emblem-system-symbolic")
    }) {
        settings.set_icon_name("emblem-system-symbolic");
    }
    settings.add_css_class("flat");
    settings.set_tooltip_text(Some("Settings"));
    settings.connect_clicked(move |_| {
        let _ = events.try_send(Event::Settings);
    });
    header.append(&settings);
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.add_css_class("flat");
    close.set_tooltip_text(Some("Close (Escape)"));
    let weak = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade() {
            window.hide();
        }
    });
    header.append(&close);
    panel.append(&header);
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Find a layout…"));
    search.add_css_class("search");
    panel.append(&search);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    if !config.drawer {
        scroll.set_min_content_height(viewport_height);
        scroll.set_max_content_height(viewport_height);
        scroll.set_propagate_natural_height(true);
    }
    let grid = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(columns)
        .max_children_per_line(columns)
        .row_spacing(8)
        .column_spacing(8)
        .build();
    grid.add_css_class("layouts");
    let mut cards = vec![];
    for layout in config.layouts() {
        let (card, button) = card(
            state,
            layout,
            &target,
            config.favorites.iter().any(|s| s == layout.name),
        );
        card.set_widget_name(layout.name);
        grid.insert(&card, -1);
        cards.push((layout, button));
    }
    let query = Rc::new(RefCell::new(String::new()));
    let filter = query.clone();
    grid.set_filter_func(move |child| {
        child.child().is_some_and(|w| {
            w.widget_name()
                .replace('_', " ")
                .contains(filter.borrow().as_str())
        })
    });
    let flow = grid.clone();
    let state_search = state.clone();
    search.connect_changed(move |search| {
        *query.borrow_mut() = search.text().to_lowercase().replace('_', " ");
        flow.invalidate_filter();
        if let Some(status) = &state_search.borrow().status {
            let matches = state_search
                .borrow()
                .cards
                .iter()
                .filter(|(l, _)| l.title.to_lowercase().contains(query.borrow().as_str()))
                .count();
            status.set_text(if matches == 0 {
                "No matching layouts. Try another name."
            } else {
                "Select a layout to apply it. ★ adds a favorite."
            });
        }
    });
    let state_enter = state.clone();
    search.connect_activate(move |search| {
        let query = search.text().to_lowercase().replace('_', " ");
        if let Some((_, button)) = state_enter
            .borrow()
            .cards
            .iter()
            .find(|(layout, _)| layout.title.to_lowercase().contains(&query))
        {
            button.emit_clicked();
        }
    });
    scroll.set_child(Some(&grid));
    panel.append(&scroll);
    let footer = vertical(5);
    footer.add_css_class("footer");
    let status = label(
        "Select a layout to apply it. ★ adds a favorite.",
        "dim-label",
    );
    status.set_wrap(true);
    footer.append(&status);
    footer.append(&label(
        "Type to filter · Arrows to browse · Enter to select · Esc to close",
        "hint",
    ));
    panel.append(&footer);
    overlay.add_overlay(&panel);
    window.set_child(Some(&overlay));
    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = window.downgrade();
    let state_key = state.clone();
    let search_key = search.clone();
    let columns = columns as usize;
    controller.connect_key_pressed(move |_, key, _, modifiers| {
        if key == gdk::Key::Escape {
            if let Some(w) = weak.upgrade() {
                w.hide();
            }
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::comma && modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
            show_settings(&state_key);
            return glib::Propagation::Stop;
        }
        let search_focused = weak
            .upgrade()
            .and_then(|w| gtk::prelude::GtkWindowExt::focus(&w))
            .is_some_and(|f| {
                f == search_key.clone().upcast::<gtk::Widget>() || f.is_ancestor(&search_key)
            });
        let delta = match key {
            gdk::Key::Down => Some(columns as isize),
            gdk::Key::Up => Some(-(columns as isize)),
            gdk::Key::Right if !search_focused => Some(1),
            gdk::Key::Left if !search_focused => Some(-1),
            _ => None,
        };
        if let Some(delta) = delta {
            let ui = state_key.borrow();
            let buttons: Vec<_> = ui
                .cards
                .iter()
                .filter(|(layout, _)| {
                    layout
                        .title
                        .to_lowercase()
                        .contains(&search_key.text().to_lowercase().replace('_', " "))
                })
                .map(|(_, b)| b)
                .collect();
            if !buttons.is_empty() {
                let next = buttons
                    .iter()
                    .position(|b| b.has_focus())
                    .map(|i| (i as isize + delta).rem_euclid(buttons.len() as isize) as usize)
                    .unwrap_or(0);
                buttons[next].grab_focus();
            }
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::slash && !search_focused {
            search_key.grab_focus();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(controller);
    let weak = window.downgrade();
    window.connect_is_active_notify(move |w| {
        if !w.is_active()
            && let Some(w) = weak.upgrade()
        {
            w.hide();
        }
    });
    {
        let mut ui = state.borrow_mut();
        ui.target = Some(target);
        ui.cards = cards;
        ui.status = Some(status);
        ui.context = Some(context);
        ui.picker = Some(window.clone());
    }
    refresh(state);
    window.present();
    search.grab_focus();
}

fn card(state: &State, layout: Layout, target: &Target, favorite: bool) -> (gtk::Box, gtk::Button) {
    let wrapper = vertical(0);
    wrapper.add_css_class("layout-card");
    let button = gtk::Button::new();
    button.add_css_class("layout-choice");
    button.set_tooltip_text(Some(layout.description));
    let content = vertical(8);
    let preview = gtk::DrawingArea::builder()
        .content_width(120)
        .content_height(76)
        .hexpand(true)
        .build();
    preview.set_draw_func(move |area, cr, width, height| {
        draw_diagram(area, cr, width, height, layout)
    });
    content.append(&preview);
    let title = label(layout.title, "layout-name");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    content.append(&title);
    button.set_child(Some(&content));
    let state_click = state.clone();
    let target = target.clone();
    button.connect_clicked(move |_| {
        let ui = state_click.borrow();
        if let Some(status) = &ui.status {
            status.remove_css_class("error");
            status.set_text(&format!("Switching to {}…", layout.title));
        }
        for (_, button) in &ui.cards {
            button.set_sensitive(false);
        }
        if let Some(jobs) = &ui.jobs {
            let _ = jobs.send(Job::Apply(target.clone(), layout));
        }
    });
    wrapper.append(&button);
    let metadata = row(4);
    metadata.add_css_class("card-meta");
    let symbol = label(&format!("[{}]", layout.symbol), "symbol");
    symbol.set_hexpand(true);
    metadata.append(&symbol);
    let star = gtk::ToggleButton::with_label("★");
    star.add_css_class("favorite");
    star.add_css_class("flat");
    star.set_active(favorite);
    star.set_tooltip_text(Some("Keep this layout at the front"));
    let state_star = state.clone();
    star.connect_toggled(move |star| {
        {
            let mut ui = state_star.borrow_mut();
            ui.config.favorites.retain(|s| s != layout.name);
            if star.is_active() {
                ui.config.favorites.push(layout.name.into());
            }
        }
        save(&state_star);
    });
    metadata.append(&star);
    wrapper.append(&metadata);
    (wrapper, button)
}

fn draw_diagram(
    area: &gtk::DrawingArea,
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    layout: Layout,
) {
    #[allow(deprecated)]
    let color = area.style_context().color();
    let width = width as f64;
    let height = height as f64;
    cr.rectangle(0., 0., width, height);
    cr.clip();
    for (index, (x, y, w, h)) in layout::diagram(layout.name).into_iter().enumerate() {
        let margin = 4.;
        let gap = 3.;
        let x = margin + x * (width - margin * 2.) + gap / 2.;
        let y = margin + y * (height - margin * 2.) + gap / 2.;
        let w = w * (width - margin * 2.) - gap;
        let h = h * (height - margin * 2.) - gap;
        cr.rectangle(x, y, w, h);
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            if index == 0 { 0.15 } else { 0.04 },
        );
        let _ = cr.fill_preserve();
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            if index == 0 { 0.9 } else { 0.42 },
        );
        cr.set_line_width(if index == 0 { 1.8 } else { 1. });
        let _ = cr.stroke();
        cr.rectangle(x + 3., y + 3., (w - 6.).max(0.), 2.);
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            0.22,
        );
        let _ = cr.fill();
    }
}

pub fn refresh(state: &State) {
    let ui = state.borrow();
    if let Some(target) = &ui.target
        && let Some(monitor) = ui.monitors.iter().find(|m| m.name == target.monitor)
    {
        let name = layout::by_symbol(&monitor.layout_symbol)
            .map(|l| l.title)
            .unwrap_or("Unknown layout");
        if let Some(context) = &ui.context {
            context.set_text(&format!(
                "{} · Tag {} · {}",
                target.monitor,
                monitor
                    .active_tags
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(" + "),
                name
            ));
        }
        for (layout, button) in &ui.cards {
            if layout.symbol.eq_ignore_ascii_case(&monitor.layout_symbol) {
                button.add_css_class("current");
            } else {
                button.remove_css_class("current");
            }
        }
        if monitor.active_tags != target.tags
            && let Some(status) = &ui.status
        {
            status.set_text("Active tags changed. Close and reopen the picker.");
        }
    }
}

pub fn show_error(state: &State, error: &str) {
    let ui = state.borrow();
    for (_, button) in &ui.cards {
        button.set_sensitive(true);
    }
    if let Some(status) = &ui.status
        && ui.picker.as_ref().is_some_and(|w| w.is_visible())
    {
        status.set_text(error);
        status.add_css_class("error");
        return;
    }
    if ui.error_window.as_ref().is_some_and(|w| w.is_visible()) {
        return;
    }
    let app = ui.app.clone();
    drop(ui);
    let window = gtk::ApplicationWindow::builder()
        .application(&app)
        .title("Mango Layout Tray")
        .default_width(440)
        .resizable(false)
        .build();
    let content = vertical(16);
    content.add_css_class("settings-content");
    content.append(&label("Could not complete that action", "subtitle"));
    let message = label(error, "error");
    message.set_wrap(true);
    message.set_max_width_chars(55);
    content.append(&message);
    let close = gtk::Button::with_label("Close");
    let weak = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(w) = weak.upgrade() {
            w.hide();
        }
    });
    content.append(&close);
    window.set_child(Some(&content));
    state.borrow_mut().error_window = Some(window.clone());
    window.present();
}

pub fn show_settings(state: &State) {
    if let Some(window) = &state.borrow().picker {
        window.hide();
    }
    if let Some(window) = &state.borrow().settings {
        window.present();
        return;
    }
    let ui = state.borrow();
    let monitor = ui
        .monitors
        .iter()
        .find(|m| m.active)
        .map(|m| m.name.as_str());
    let window = layer_window(&ui.app, monitor);
    window.set_title(Some("Mango · Settings"));
    let content = vertical(18);
    content.add_css_class("settings-content");
    content.append(&label("Make it yours.", "title"));
    let modes = row(12);
    let text = vertical(4);
    text.set_hexpand(true);
    text.append(&label("Open as a drawer", "subtitle"));
    text.append(&label("More room for previews", "dim-label"));
    modes.append(&text);
    let drawer = gtk::Switch::builder()
        .active(ui.config.drawer)
        .valign(gtk::Align::Center)
        .build();
    modes.append(&drawer);
    content.append(&modes);
    let st = state.clone();
    drawer.connect_active_notify(move |switch| {
        st.borrow_mut().config.drawer = switch.is_active();
        save(&st);
    });
    let theme_row = row(12);
    let text = label("Appearance", "subtitle");
    text.set_hexpand(true);
    theme_row.append(&text);
    let theme = gtk::DropDown::from_strings(&["GTK theme", "Light", "Dark"]);
    theme.set_selected(match ui.config.theme {
        Theme::System => 0,
        Theme::Light => 1,
        Theme::Dark => 2,
    });
    theme_row.append(&theme);
    content.append(&theme_row);
    let st = state.clone();
    theme.connect_selected_notify(move |dropdown| {
        {
            let mut ui = st.borrow_mut();
            ui.config.theme = match dropdown.selected() {
                1 => Theme::Light,
                2 => Theme::Dark,
                _ => Theme::System,
            };
            ui.theme();
        }
        save(&st);
    });
    let startup = row(12);
    let text = label("Start with the session", "subtitle");
    text.set_hexpand(true);
    startup.append(&text);
    let start = gtk::Switch::builder()
        .active(config::autostart_enabled().unwrap_or(false))
        .valign(gtk::Align::Center)
        .build();
    startup.append(&start);
    content.append(&startup);
    let st = state.clone();
    start.connect_state_set(move |_, enabled| {
        if let Err(e) = config::set_autostart(enabled) {
            show_error(&st, &format!("Could not update autostart: {e:#}"));
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    let note = label(
        "Adds an exec-once entry to your Mango configuration. Takes effect next session; disabling removes only the entry added here.",
        "hint",
    );
    note.set_wrap(true);
    content.append(&note);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&label("Layout order", "subtitle"));
    content.append(&label(
        "Favorites appear first. Use the arrows to reorder.",
        "dim-label",
    ));
    let order = vertical(3);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .min_content_height(200)
        .child(&order)
        .build();
    content.append(&scroll);
    let close = gtk::Button::with_label("Done");
    close.add_css_class("suggested-action");
    let weak = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(w) = weak.upgrade() {
            w.hide();
        }
    });
    content.append(&close);
    let overlay = gtk::Overlay::new();
    let backdrop = gtk::Button::new();
    backdrop.add_css_class("backdrop");
    backdrop.set_can_focus(false);
    let weak = window.downgrade();
    backdrop.connect_clicked(move |_| {
        if let Some(w) = weak.upgrade() {
            w.hide();
        }
    });
    overlay.set_child(Some(&backdrop));
    content.add_css_class("picker");
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    content.set_size_request(440, 660);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    overlay.add_overlay(&content);
    window.set_child(Some(&overlay));
    drop(ui);
    build_order(state, &order);
    state.borrow_mut().settings = Some(window.clone());
    window.connect_close_request(|w| {
        w.hide();
        glib::Propagation::Stop
    });
    let keys = gtk::EventControllerKey::new();
    let weak = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            if let Some(w) = weak.upgrade() {
                w.hide();
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
    window.present();
}

fn build_order(state: &State, container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    let layouts = state.borrow().config.layouts();
    for (index, layout) in layouts.iter().enumerate() {
        let item = row(8);
        let name = label(layout.title, "");
        name.set_hexpand(true);
        item.append(&name);
        for (delta, icon, help) in [
            (-1isize, "go-up-symbolic", "Move up"),
            (1, "go-down-symbolic", "Move down"),
        ] {
            let button = gtk::Button::from_icon_name(icon);
            button.add_css_class("flat");
            button.set_tooltip_text(Some(help));
            let next = index as isize + delta;
            let favorites = &state.borrow().config.favorites;
            button.set_sensitive(
                next >= 0
                    && (next as usize) < layouts.len()
                    && favorites.iter().any(|f| f == layout.name)
                        == favorites.iter().any(|f| f == layouts[next as usize].name),
            );
            let st = state.clone();
            let list = container.clone();
            button.connect_clicked(move |_| {
                {
                    let mut ui = st.borrow_mut();
                    let mut layouts = ui.config.layouts();
                    let next = (index as isize + delta) as usize;
                    layouts.swap(index, next);
                    ui.config.order = layouts.iter().map(|l| l.name.into()).collect();
                    let favorites = ui.config.favorites.clone();
                    ui.config.favorites = layouts
                        .iter()
                        .filter(|l| favorites.iter().any(|f| f == l.name))
                        .map(|l| l.name.into())
                        .collect();
                }
                save(&st);
                build_order(&st, &list);
            });
            item.append(&button);
        }
        container.append(&item);
    }
}
