mod config;
mod ipc;
mod layout;
mod tray;
mod ui;

use async_channel::Sender;
use gtk::{gio, glib, prelude::*};
use ipc::{Ipc, Monitor, Target};
use ksni::blocking::TrayMethods;
use layout::Layout;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Debug)]
enum Event {
    Open(bool),
    Opened(Target, Vec<Monitor>),
    Quick(Layout),
    Settings,
    Quit,
    State(Vec<Monitor>),
    Error(String),
    Applied,
}

enum Job {
    Open(bool),
    Apply(Target, Layout),
    Quick(Layout),
}

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|s| s == "--version") {
        println!("mango-layout-tray {}", env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    if args.iter().any(|s| s == "--check") {
        return match check() {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e:#}");
                glib::ExitCode::FAILURE
            }
        };
    }
    let app = gtk::Application::builder()
        .application_id("io.github.mikkelrask.MangoLayoutTray")
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    for (name, help) in [
        ("show", "Open the layout picker on the focused monitor"),
        ("settings", "Open settings"),
        ("quit", "Quit the running app"),
    ] {
        app.add_main_option(
            name,
            glib::Char::from(0),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            help,
            None,
        );
    }
    let (tx, rx) = async_channel::unbounded();
    let initialized = Rc::new(Cell::new(false));
    let init = initialized.clone();
    let events = tx.clone();
    app.connect_command_line(move |app, command| {
        let options = command.options_dict();
        if options.contains("quit") {
            app.quit();
            return glib::ExitCode::SUCCESS;
        }
        app.activate();
        if options.contains("settings") {
            let _ = events.try_send(Event::Settings);
        } else if options.contains("show") || init.replace(true) {
            let _ = events.try_send(Event::Open(false));
        }
        glib::ExitCode::SUCCESS
    });
    let started = Rc::new(Cell::new(false));
    app.connect_activate(move |app| {
        if started.replace(true) {
            return;
        }
        let hold = app.hold();
        let initial_config = config::Config::load();
        let initial_error = initial_config.as_ref().err().map(|e| format!("{e:#}"));
        let state = Rc::new(RefCell::new(ui::Ui::new(
            app.clone(),
            initial_config.unwrap_or_default(),
            tx.clone(),
        )));
        if let Some(error) = initial_error {
            let _ = tx.try_send(Event::Error(error));
        }
        let (jobs, work) = std::sync::mpsc::channel();
        state.borrow_mut().jobs = Some(jobs.clone());
        start_workers(tx.clone(), work);
        let tray = tray::Tray {
            symbol: "?".into(),
            monitor: String::new(),
            error: None,
            events: tx.clone(),
        };
        let tray = match tray.spawn() {
            Ok(handle) => Some(handle),
            Err(e) => {
                let _ = tx.try_send(Event::Error(format!("Cannot register system tray: {e}")));
                None
            }
        };
        let receiver = rx.clone();
        let application = app.clone();
        glib::spawn_future_local(async move {
            let _hold = hold;
            while let Ok(event) = receiver.recv().await {
                match event {
                    Event::Open(pointer) => {
                        let _ = jobs.send(Job::Open(pointer));
                    }
                    Event::Quick(layout) => {
                        let _ = jobs.send(Job::Quick(layout));
                    }
                    Event::Opened(target, monitors) => {
                        state.borrow_mut().monitors = monitors;
                        ui::show_picker(&state, target);
                    }
                    Event::Settings => ui::show_settings(&state),
                    Event::Quit => {
                        application.quit();
                        break;
                    }
                    Event::Applied => {
                        let s = state.borrow();
                        if let Some(window) = &s.picker {
                            window.hide();
                        }
                    }
                    Event::Error(error) => {
                        eprintln!("{error}");
                        if let Some(tray) = &tray {
                            let text = error.clone();
                            tray.update(|t| t.error = Some(text));
                        }
                        ui::show_error(&state, &error);
                    }
                    Event::State(monitors) => {
                        if let Some(monitor) = monitors.iter().find(|m| m.active)
                            && let Some(tray) = &tray
                        {
                            let symbol = monitor.layout_symbol.clone();
                            let name = monitor.name.clone();
                            tray.update(|t| {
                                t.symbol = symbol;
                                t.monitor = name;
                                t.error = None;
                            });
                        }
                        state.borrow_mut().monitors = monitors;
                        ui::refresh(&state);
                    }
                }
            }
        });
    });
    app.run()
}

fn start_workers(events: Sender<Event>, jobs: std::sync::mpsc::Receiver<Job>) {
    let update = events.clone();
    std::thread::spawn(move || {
        loop {
            let result = Ipc::from_env().and_then(|ipc| {
                let state = ipc.monitors()?;
                if update.send_blocking(Event::State(state)).is_err() {
                    return Ok(());
                }
                ipc.watch(|monitors| update.send_blocking(Event::State(monitors)).is_ok())
            });
            if let Err(error) = result {
                if update
                    .send_blocking(Event::Error(format!("{error:#}")))
                    .is_err()
                {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            } else {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        while let Ok(job) = jobs.recv() {
            let result = Ipc::from_env().and_then(|ipc| {
                match job {
                    Job::Open(pointer) => {
                        let target = ipc.target(pointer)?;
                        let monitors = ipc.monitors()?;
                        let _ = events.send_blocking(Event::Opened(target, monitors));
                    }
                    Job::Apply(target, layout) => {
                        ipc.apply(&target, layout.name)?;
                        let _ = events.send_blocking(Event::Applied);
                    }
                    Job::Quick(layout) => {
                        let target = ipc.target(true)?;
                        ipc.apply(&target, layout.name)?;
                    }
                }
                Ok(())
            });
            if let Err(e) = result {
                let _ = events.send_blocking(Event::Error(format!("{e:#}")));
            }
        }
    });
}

fn check() -> anyhow::Result<()> {
    let config = config::Config::load()?;
    let ipc = Ipc::from_env()?;
    let monitors = ipc.monitors()?;
    let layouts = ipc.request("get layouts")?;
    println!(
        "MangoWM IPC connected. {} monitors, {} configured layouts.\n{}",
        monitors.len(),
        config.layouts().len(),
        serde_json::to_string_pretty(&layouts)?
    );
    Ok(())
}
