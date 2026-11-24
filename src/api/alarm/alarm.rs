use std::sync::Mutex;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Sense, Stroke, ViewportCommand};

const BALL_SIZE: [f32; 2] = [64.0, 64.0];
const BALL_RADIUS: f32 = 29.0;
const RIPPLE_COUNT: u128 = 3;
const RIPPLE_SPACING_MS: u128 = 900; // 420
const RIPPLE_FRAME_INTERVAL: Duration = Duration::from_millis(16);
const IDLE_FILL: Color32 = Color32::from_rgb(16, 25, 43);
const IDLE_ICON: Color32 = Color32::from_rgb(114, 218, 255);
const ALARM_ACCENT: Color32 = Color32::from_rgb(255, 202, 81);

struct Signal {
    started_at: Option<Instant>,
    context: Option<egui::Context>,
}

static SIGNAL: Mutex<Signal> = Mutex::new(Signal {
    started_at: None,
    context: None,
});

/// Starts the visual alarm. Safe to call from any thread, including before [`run`].
/// Waves keep rippling outward until the user clicks the ball. Repeated calls leave it active.
pub fn flash() {
    let context = {
        let mut signal = SIGNAL.lock().expect("alarm signal mutex poisoned");
        if signal.started_at.is_some() {
            return;
        }
        signal.started_at = Some(Instant::now());
        signal.context.clone()
    };
    if let Some(context) = context {
        context.request_repaint();
    }
}

/// Opens the always-on-top alarm ball and runs its event loop until it closes.
/// Call on the desktop application's main thread.
pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(BALL_SIZE)
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_has_shadow(false)
            .with_taskbar(false) // 不在任务栏显示
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "Silent Alarm Ball",
        options,
        Box::new(|creation| Ok(Box::new(AlarmBall::new(&creation.egui_ctx)))),
    )
}

struct AlarmBall;

impl AlarmBall {
    fn new(context: &egui::Context) -> Self {
        SIGNAL.lock().expect("alarm signal mutex poisoned").context = Some(context.clone());
        Self
    }
}

impl Drop for AlarmBall {
    fn drop(&mut self) {
        SIGNAL.lock().expect("alarm signal mutex poisoned").context = None;
    }
}

impl eframe::App for AlarmBall {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let button = ui.interact(rect, ui.id().with("alarm-ball"), Sense::click_and_drag());
                if button.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }

                let center = rect.center();
                let hovered = ui.input(|input| {
                    input
                        .pointer
                        .hover_pos()
                        .is_some_and(|pos| pos.distance(center) <= BALL_RADIUS)
                });
                let started = {
                    let mut signal = SIGNAL.lock().expect("alarm signal mutex poisoned");
                    if hovered && button.clicked_by(egui::PointerButton::Primary) {
                        signal.started_at = None;
                    }
                    signal.started_at
                };

                let mut painter = ui.painter().clone();
                let (border, icon) = if started.is_some() {
                    (ALARM_ACCENT.gamma_multiply(0.48), ALARM_ACCENT)
                } else {
                    if !hovered {
                        painter.multiply_opacity(0.5);
                    }
                    (IDLE_ICON, IDLE_ICON)
                };

                painter.circle(center, BALL_RADIUS, IDLE_FILL, Stroke::new(1.5_f32, border));
                if let Some(started) = started {
                    paint_ripples(&painter, center, started.elapsed());
                    ctx.request_repaint_after(RIPPLE_FRAME_INTERVAL);
                }
                paint_bell(&painter, center, icon);
            });
    }
}

fn paint_ripples(painter: &egui::Painter, center: egui::Pos2, elapsed: Duration) {
    let elapsed_ms = elapsed.as_millis();
    let lifetime_ms = RIPPLE_SPACING_MS * RIPPLE_COUNT;
    for index in 0..RIPPLE_COUNT {
        let Some(age_ms) = elapsed_ms.checked_sub(index * RIPPLE_SPACING_MS) else {
            break;
        };
        let progress = (age_ms % lifetime_ms) as f32 / lifetime_ms as f32;
        let radius = egui::lerp(1.5..=(BALL_RADIUS - 1.5), progress);
        let width = egui::lerp(1.8..=0.9, progress);
        let color = ALARM_ACCENT.gamma_multiply(0.9 * (1.0 - progress));
        painter.circle_stroke(center, radius, Stroke::new(width, color));
    }
}

fn paint_bell(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let outline = [
        (-11.0, 6.0),
        (-8.0, 1.0),
        (-8.0, -4.0),
        (-6.0, -8.0),
        (-2.0, -10.0),
        (2.0, -10.0),
        (6.0, -8.0),
        (8.0, -4.0),
        (8.0, 1.0),
        (11.0, 6.0),
        (-11.0, 6.0),
    ];
    let points = outline
        .into_iter()
        .map(|(x, y)| center + egui::vec2(x, y))
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(1.75_f32, color)));
    painter.circle_filled(center + egui::vec2(0.0, 10.0), 1.7, color);
}

#[cfg(test)]
#[path = "silent_alarm_tests.rs"]
mod tests;
