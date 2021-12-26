//! # Sitter
//!
//! `sitter` is a small desktop application that helps remind you to get up
//! when you've been sitting for too long.

mod dbus;
mod ui;

use std::process::{Command, Stdio};
use std::str::from_utf8;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use clap::{App, Arg};
use druid::{AppLauncher, Size, WindowDesc};

fn main() {
    let matches = App::new("sitter")
        .version("0.1.0")
        .about("Reminds you to get up after you've been sitting for too long.")
        .arg(
            Arg::with_name("duration")
                .short("d")
                .long("duration")
                .help("minutes the timer should run")
                .default_value("30"),
        )
        .arg(
            Arg::with_name("snooze")
                .short("s")
                .long("snooze")
                .help("minutes the timer should snooze")
                .default_value("5"),
        )
        .get_matches();

    let get_seconds = move |name: &str| {
        matches
            .value_of(name)
            .unwrap()
            .parse::<f32>()
            .map(|mins| (mins * 60.0).round() as u64)
            .unwrap_or_else(|_| panic!("Invalid {}", name))
    };

    let (tx, rx) = mpsc::channel();
    dbus::watch(tx);

    let ui_handle = thread::spawn(move || {
        let main_window = WindowDesc::new(ui::root)
            .title("sitter")
            .show_titlebar(false)
            .with_min_size(Size::new(0.0, 0.0));
        let data = ui::AppState::new(rx)
            .with_duration(get_seconds("duration"))
            .with_snooze(get_seconds("snooze"));
        AppLauncher::with_window(main_window)
            .use_simple_logger()
            .launch(data)
            .unwrap();
    });

    loop {
        let cmd = Command::new("wmctrl")
            .args(vec!["-l", "-x", "-p"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("wmctrl failed")
            .wait_with_output()
            .expect("Failed to wait on child");
        let stdout = from_utf8(&cmd.stdout).unwrap();
        if stdout.contains("sitter.Sitter") {
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }

    Command::new("wmctrl")
        .args(vec![
            "-r",
            "sitter.Sitter",
            "-x",
            "-b",
            "add,above,sticky",
            "-v",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("wmctrl failed");

    ui_handle.join().unwrap();
}
