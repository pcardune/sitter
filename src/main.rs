use std::io::{self, BufRead};
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::{self, Duration, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DBusEvent {
    time: SystemTime,
    member: String,
}

impl DBusEvent {
    fn new(member: String) -> DBusEvent {
        DBusEvent {
            time: time::SystemTime::now(),
            member,
        }
    }
}

fn parse_dbus_line(line: &str) -> Option<DBusEvent> {
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
        member.map(DBusEvent::new)
    } else {
        None
    }
}

fn format_time(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{} seconds", secs)
    } else {
        format!("{}:{:02} minutes", secs / 60, secs % 60)
    }
}

fn watch_dbus(tx: mpsc::Sender<DBusEvent>) -> thread::JoinHandle<()> {
    let mut child = process::Command::new("dbus-monitor")
        .arg("--session")
        .arg("type='signal',interface='org.gnome.ScreenSaver'")
        .stdout(process::Stdio::piped())
        .spawn()
        .expect("Failed to start dbus-monitor");
    let stdout = child.stdout.take().unwrap();

    thread::spawn(move || {
        let mut reader = io::BufReader::new(stdout);
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(event) = parse_dbus_line(&line) {
                println!("Event={:?}", event);
                tx.send(event).unwrap();
            }
            println!("{}", line);
        }
    })
}

use std::rc::Rc;

use druid::commands::SHOW_WINDOW;
use druid::widget::{Button, Controller, Either, Flex, Label, LabelText};
use druid::{
    AppLauncher, Data, Env, Event, EventCtx, PlatformError, TimerToken, Widget, WidgetExt,
    WindowDesc,
};

fn make_label<T: Data>(f: impl Into<LabelText<T>>) -> impl Widget<T> {
    Label::new(f).padding(5.0).center()
}

fn ui_builder() -> impl Widget<AppState> {
    let button = Button::new(|data: &AppState, _env: &_| {
        format!("Snooze {}", format_time(data.snooze_duration))
    })
    .on_click(|_ctx, data: &mut AppState, _env| data.snooze())
    .padding(5.0);

    let button = Either::new(
        |data: &AppState, _env: &_| data.is_past_due(),
        button,
        Label::new(""),
    );

    Flex::column()
        .with_child(make_label(|data: &AppState, _env: &_| {
            format!("Duration: {}", format_time(data.timer_duration),)
        }))
        .with_child(make_label(|data: &AppState, _env: &_| {
            format!("Remaining: {}", format_time(data.remaining()),)
        }))
        .with_child(
            Label::new(|data: &AppState, _env: &_| {
                format!("{} since you sat down", format_time(data.elapsed()))
            })
            .padding(5.0)
            .center(),
        )
        .with_child(button)
        .controller(UpdateEvent::new())
}

struct UpdateEvent {
    timer_id: TimerToken,
    timer_interval: Duration,
}
impl UpdateEvent {
    fn new() -> UpdateEvent {
        UpdateEvent {
            timer_id: TimerToken::INVALID,
            timer_interval: Duration::from_millis(1000),
        }
    }
}
impl<W: Widget<AppState>> Controller<AppState, W> for UpdateEvent {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut AppState,
        env: &Env,
    ) {
        match event {
            Event::WindowConnected => {
                // Start the timer when the application launches
                self.timer_id = ctx.request_timer(self.timer_interval);
            }
            Event::Timer(id) => {
                if *id == self.timer_id {
                    ctx.request_layout();
                    self.timer_id = ctx.request_timer(self.timer_interval);
                    data.count += 1;
                    data.update();
                    if data.is_past_due() {
                        ctx.submit_command(SHOW_WINDOW)
                    }
                }
            }
            _ => {}
        }

        child.event(ctx, event, data, env)
    }
}

#[derive(Data, Clone)]
struct AppState {
    dbus_receiver: Rc<mpsc::Receiver<DBusEvent>>,

    count: u32,
    #[data(same_fn = "PartialEq::eq")]
    last_event: DBusEvent,

    #[data(same_fn = "PartialEq::eq")]
    timer_duration: Duration,

    #[data(same_fn = "PartialEq::eq")]
    last_snooze: Option<SystemTime>,

    #[data(same_fn = "PartialEq::eq")]
    snooze_duration: Duration,
}
impl AppState {
    fn new(rx: mpsc::Receiver<DBusEvent>) -> AppState {
        AppState {
            count: 0,
            last_event: DBusEvent::new("start".into()),
            dbus_receiver: Rc::new(rx),
            timer_duration: Duration::new(30 * 60, 0),
            last_snooze: None,
            snooze_duration: Duration::new(5 * 60, 0),
        }
    }
    fn with_duration(mut self, seconds: u64) -> AppState {
        self.timer_duration = Duration::new(seconds, 0);
        self
    }
    fn with_snooze(mut self, seconds: u64) -> AppState {
        self.snooze_duration = Duration::new(seconds, 0);
        self
    }
    fn update(&mut self) {
        if let Ok(event) = self.dbus_receiver.try_recv() {
            if event.member == "WakeUpScreen" {
                self.last_event = event;
                self.last_snooze = None;
            }
        }
    }
    fn snooze(&mut self) {
        self.last_snooze = Some(SystemTime::now());
    }

    fn is_past_due(&self) -> bool {
        self.remaining().is_zero()
    }

    fn elapsed(&self) -> Duration {
        self.last_event.time.elapsed().unwrap()
    }

    fn remaining(&self) -> Duration {
        if let Some(last_snooze) = self.last_snooze {
            let last_snooze = last_snooze.elapsed().unwrap();
            if self.snooze_duration > last_snooze {
                self.snooze_duration - last_snooze
            } else {
                Duration::default()
            }
        } else if self.timer_duration > self.elapsed() {
            self.timer_duration - self.elapsed()
        } else {
            Duration::default()
        }
    }
}

fn main() -> Result<(), PlatformError> {
    use clap::{App, Arg};
    let matches = App::new("Timer")
        .arg(
            Arg::with_name("duration")
                .short("d")
                .help("minutes the timer should run")
                .default_value("30"),
        )
        .arg(
            Arg::with_name("snooze")
                .short("s")
                .help("minutes the timer should snooze")
                .default_value("5"),
        )
        .get_matches();

    let get_seconds = |name: &str| {
        matches
            .value_of(name)
            .unwrap()
            .parse::<f32>()
            .map(|mins| (mins * 60.0).round() as u64)
            .unwrap_or_else(|_| panic!("Invalid {}", name))
    };

    let (tx, rx) = mpsc::channel();
    watch_dbus(tx);
    let main_window = WindowDesc::new(ui_builder).title("Timer");
    let data = AppState::new(rx)
        .with_duration(get_seconds("duration"))
        .with_snooze(get_seconds("snooze"));
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(data)
}
