//! Module for working with dbus. See https://www.freedesktop.org/wiki/Software/dbus/
//!

use std::io::{BufRead, BufReader};
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::SystemTime;

/// Struct representing an event from dbus
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The time the event took place
    pub time: SystemTime,

    /// The member field of the event, e.g. "WakeUpScreen"
    pub member: String,
}

impl Event {
    pub fn new(member: String) -> Event {
        Event {
            time: SystemTime::now(),
            member,
        }
    }
}

fn parse_dbus_line(line: &str) -> Option<Event> {
    let mut segments = line.split_whitespace();
    if segments.next() == Some("signal") {
        let mut member: Option<String> = None;
        for segment in segments {
            let mut parts = segment.split("=");
            match parts.next() {
                Some("member") => {
                    member = Some(parts.next().unwrap().into());
                }
                _ => {}
            }
        }
        member.map(Event::new)
    } else {
        None
    }
}

/// Run dbus-monitor in a child process and send a signal
/// every time there is an org.gnome.ScreenSaver event
pub fn watch(tx: mpsc::Sender<Event>) -> thread::JoinHandle<()> {
    let mut child = process::Command::new("dbus-monitor")
        .arg("--session")
        .arg("type='signal',interface='org.gnome.ScreenSaver'")
        .stdout(process::Stdio::piped())
        .spawn()
        .expect("Failed to start dbus-monitor");
    let stdout = child.stdout.take().unwrap();

    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(event) = parse_dbus_line(&line) {
                tx.send(event).unwrap();
            }
        }
    })
}
