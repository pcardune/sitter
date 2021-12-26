#![allow(dead_code)]

use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use druid::commands::SHOW_WINDOW;
use druid::widget::{Button, Controller, Either, Flex, Label, LabelText};
use druid::{Color, Data, Env, Event, EventCtx, Point, Size, TimerToken, Widget, WidgetExt};

use crate::dbus;

static COLLAPSED_BACKGROUND: Color = Color::rgb8(10, 147, 150);
static EXPANDED_BACKGROUND: Color = Color::rgb8(174, 32, 18);
static COLLAPSED_BORDER: Color = Color::rgb8(0, 18, 25);
static COLLAPSED_SIZE: Size = Size::new(200.0, 30.0);
static EXPANDED_SIZE: Size = Size::new(500.0, 500.0);

fn format_time(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{} seconds", secs)
    } else {
        format!("{}:{:02} minutes", secs / 60, secs % 60)
    }
}

fn make_label<T: Data>(f: impl Into<LabelText<T>>) -> impl Widget<T> {
    Label::new(f).padding(5.0).center()
}

fn expand(ctx: &mut EventCtx, data: &mut AppState, _env: &Env) {
    data.collapsed = false;
    data.collapsed_window_pos = ctx.window().get_position();
    ctx.window().set_size(EXPANDED_SIZE);
}
fn collapse(ctx: &mut EventCtx, data: &mut AppState, _env: &Env) {
    data.collapsed = true;
    ctx.window().set_size(COLLAPSED_SIZE);
    ctx.window().set_position(data.collapsed_window_pos);
}

fn snooze(ctx: &mut EventCtx, data: &mut AppState, env: &Env) {
    data.snooze();
    collapse(ctx, data, env);
}
fn ui_collapsed() -> impl Widget<AppState> {
    Label::new(|data: &AppState, _env: &_| format_time(data.remaining()))
        .padding(5.0)
        .center()
        .on_click(expand)
        .background(COLLAPSED_BACKGROUND.clone())
}
fn ui_expanded() -> impl Widget<AppState> {
    let buttons = Either::new(
        |data: &AppState, _env: &_| data.is_past_due(),
        Button::new(|data: &AppState, _env: &_| {
            format!("Snooze {}", format_time(data.snooze_duration))
        })
        .on_click(snooze)
        .padding(5.0),
        Button::new("Collapse").on_click(collapse).padding(5.0),
    );

    Flex::column()
        .with_child(
            Label::new(|data: &AppState, _env: &_| {
                format!("{} since you sat down", format_time(data.elapsed()))
            })
            .padding(5.0)
            .center(),
        )
        .with_child(make_label(|data: &AppState, _env: &_| {
            format!("Duration: {}", format_time(data.timer_duration),)
        }))
        .with_child(make_label(|data: &AppState, _env: &_| {
            format!("Remaining: {}", format_time(data.remaining()),)
        }))
        .with_child(buttons)
        .background(EXPANDED_BACKGROUND.clone())
}

pub fn root() -> impl Widget<AppState> {
    Either::new(
        |data, _env: &_| data.collapsed,
        ui_collapsed(),
        ui_expanded(),
    )
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
                ctx.window().set_position(Point::ZERO);
                ctx.window().set_size(EXPANDED_SIZE);
                ctx.request_layout()
            }
            Event::Timer(id) => {
                if *id == self.timer_id {
                    ctx.request_layout();
                    self.timer_id = ctx.request_timer(self.timer_interval);
                    data.count += 1;
                    if let Ok(event) = data.dbus_receiver.try_recv() {
                        if event.member == "WakeUpScreen" {
                            data.last_event = event;
                            data.last_snooze = None;
                        }
                    }
                    if data.is_past_due() {
                        if data.collapsed {
                            expand(ctx, data, env);
                        }
                        ctx.submit_command(SHOW_WINDOW);
                    }
                }
            }
            _ => {}
        }

        child.event(ctx, event, data, env)
    }
}

#[derive(Data, Clone)]
pub struct AppState {
    dbus_receiver: Rc<mpsc::Receiver<dbus::Event>>,

    count: u32,
    #[data(same_fn = "PartialEq::eq")]
    last_event: dbus::Event,

    #[data(same_fn = "PartialEq::eq")]
    timer_duration: Duration,

    #[data(same_fn = "PartialEq::eq")]
    last_snooze: Option<SystemTime>,

    #[data(same_fn = "PartialEq::eq")]
    snooze_duration: Duration,

    collapsed: bool,
    collapsed_window_pos: Point,
}
impl AppState {
    pub fn new(rx: mpsc::Receiver<dbus::Event>) -> AppState {
        AppState {
            count: 0,
            last_event: dbus::Event::new("start".into()),
            dbus_receiver: Rc::new(rx),
            timer_duration: Duration::new(30 * 60, 0),
            last_snooze: None,
            snooze_duration: Duration::new(5 * 60, 0),
            collapsed: false,
            collapsed_window_pos: Point::ZERO,
        }
    }
    pub fn with_duration(mut self, seconds: u64) -> AppState {
        self.timer_duration = Duration::new(seconds, 0);
        self
    }
    pub fn with_snooze(mut self, seconds: u64) -> AppState {
        self.snooze_duration = Duration::new(seconds, 0);
        self
    }
    fn snooze(&mut self) {
        self.last_snooze = Some(SystemTime::now());
    }
    fn is_snoozing(&self) -> bool {
        self.last_snooze.is_some()
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
