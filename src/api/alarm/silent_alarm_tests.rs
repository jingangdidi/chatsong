use super::*;
use eframe::App;
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, ViewportId};

#[test]
fn concentric_rings_start_in_sequence_expand_and_fade_inside_the_ball() {
    let ctx = egui::Context::default();
    let center = egui::pos2(32.0, 32.0);
    let rings_at = |millis| {
        ctx.run(RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("ripples"));
            paint_ripples(
                &ctx.layer_painter(layer),
                center,
                Duration::from_millis(millis),
            );
        })
        .shapes
        .into_iter()
        .filter_map(|shape| match shape.shape {
            egui::Shape::Circle(ring) => Some(ring),
            _ => None,
        })
        .collect::<Vec<_>>()
    };

    let start = rings_at(0);
    assert_eq!(start.len(), 1);
    assert_eq!(rings_at(419).len(), 1);
    let second = rings_at(420);
    assert_eq!(second.len(), 2);
    assert!(second[0].radius > start[0].radius);
    assert_eq!(second[1].radius, start[0].radius);

    let near_edge = rings_at(1230);
    assert_eq!(near_edge.len(), 3);
    assert!(near_edge.iter().all(|ring| ring.center == center));
    assert!(
        near_edge
            .windows(2)
            .all(|pair| pair[0].radius > pair[1].radius)
    );
    assert!(near_edge[0].radius + near_edge[0].stroke.width / 2.0 < BALL_RADIUS);
    assert!(near_edge[0].stroke.color.a() < second[0].stroke.color.a());
    assert_eq!(rings_at(1260)[0].radius, start[0].radius);
}

#[test]
fn a_click_acknowledges_an_alarm_and_the_next_alarm_can_start() {
    let ctx = egui::Context::default();
    flash();
    let first_alarm = active_alarm().expect("an alert is pending before startup");
    let mut ball = AlarmBall::new(&ctx);
    render(&mut ball, &ctx, vec![]);

    std::thread::spawn(flash).join().unwrap();
    assert_eq!(active_alarm(), Some(first_alarm));

    let center = egui::pos2(32.0, 32.0);
    let end = egui::pos2(52.0, 46.0);
    render(
        &mut ball,
        &ctx,
        vec![Event::PointerMoved(center), press(center, true)],
    );
    let commands = render(&mut ball, &ctx, vec![Event::PointerMoved(end)]);
    assert!(commands.contains(&ViewportCommand::StartDrag));
    render(&mut ball, &ctx, vec![press(end, false)]);
    assert_eq!(active_alarm(), Some(first_alarm));

    assert!(click(&mut ball, &ctx, egui::pos2(2.0, 2.0)).is_empty());
    assert_eq!(active_alarm(), Some(first_alarm));
    assert!(click(&mut ball, &ctx, center).is_empty());
    assert_eq!(active_alarm(), None);
    assert!(click(&mut ball, &ctx, center).is_empty());

    std::thread::spawn(flash).join().unwrap();
    assert!(active_alarm().is_some());
    assert!(click(&mut ball, &ctx, center).is_empty());
    assert_eq!(active_alarm(), None);
}

fn active_alarm() -> Option<Instant> {
    SIGNAL.lock().unwrap().started_at
}

fn click(ball: &mut AlarmBall, ctx: &egui::Context, pos: Pos2) -> Vec<ViewportCommand> {
    let mut commands = render(ball, ctx, vec![Event::PointerMoved(pos), press(pos, true)]);
    commands.extend(render(ball, ctx, vec![press(pos, false)]));
    commands
}

fn press(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn render(ball: &mut AlarmBall, ctx: &egui::Context, events: Vec<Event>) -> Vec<ViewportCommand> {
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, BALL_SIZE.into())),
            events,
            ..Default::default()
        },
        |ctx| ball.update(ctx, &mut eframe::Frame::_new_kittest()),
    );
    output.viewport_output[&ViewportId::ROOT].commands.clone()
}
