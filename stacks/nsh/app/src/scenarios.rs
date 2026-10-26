//! The reference app's Rust components (they run on NSH's engine thread).

use std::{cell::RefCell, rc::Rc, sync::Arc, sync::atomic::Ordering::Relaxed, time::Duration};

use nsh_core::{
    Bounds, ModalAnimation, Point, Points, Size,
    canvas::CanvasGradient,
    color::rgb,
    ui::{ScrollEvent, prelude::*},
};

use crate::shared::*;

fn pt(x: f32, y: f32) -> Point<Points> {
    Point::new(px(x), px(y))
}

fn col(hex: u32) -> Hsla {
    rgb(hex).into()
}

// Palette (light shell).
const BG: u32 = 0xf6f7f9;
const BAR: u32 = 0x1f2430;
const ROW_A: u32 = 0xffffff;
const ROW_B: u32 = 0xf1f3f7;
const TEXT: u32 = 0x1b1f27;
const MUTED: u32 = 0x6a7280;

/// The shell around a body: title bar, body, status bar.
fn chrome(body: impl IntoElement, status: String) -> impl IntoElement {
    view()
        .flex_1()
        .bg(col(BG))
        .child(
            view()
                .h(px(40.0))
                .flex_shrink_0()
                .bg(col(BAR))
                .justify_center()
                .px(px(16.0))
                .child(
                    text("NSH bench")
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(col(0xffffff)),
                ),
        )
        .child(view().flex_1().child(body))
        .child(
            view()
                .h(px(24.0))
                .flex_shrink_0()
                .bg(col(0xe3e6ec))
                .justify_center()
                .px(px(16.0))
                .child(text(status).text_size(px(12.0)).text_color(col(MUTED))),
        )
}

/// Runs `f` once, on the first engine frame of the measured window.
fn at_measure_start(f: impl FnOnce() + 'static) {
    std::mem::forget(spawn(async move {
        loop {
            next_frame().await;
            if measured_t().is_some() {
                break;
            }
        }
        f();
    }));
}

/// The scenario root.
#[component]
pub fn Bench() -> impl IntoElement {
    let sc = cfg().scenario.clone();
    let rows = "100000 rows".to_owned();
    let root: AnyElement = match sc.as_str() {
        "idle" => chrome(list_pane(false, false), rows).into_element(),
        "list" => chrome(list_pane(true, false), rows).into_element(),
        "stream" => chrome(list_pane(true, true), rows).into_element(),
        "input" => {
            // The square over the scrolling list; Space toggles black / white through
            // the app's keymap (`on_key`, docs: nsh/keymap), the normal key path.
            let black = signal(true);
            on_key("space", move |_| {
                KEY_EVENTS.fetch_add(1, Relaxed);
                black.update(|b| *b = !*b);
            });
            view()
                .flex_1()
                .child(chrome(list_pane(true, false), rows))
                .child(
                    view()
                        .absolute()
                        .left(px(100.0))
                        .top(px(100.0))
                        .w(px(200.0))
                        .h(px(200.0))
                        .bg(move || if black.get() { col(0x000000) } else { col(0xffffff) }),
                )
                .into_element()
        }
        "canvas" => view()
            .flex_1()
            .bg(col(BG))
            .p(px(10.0))
            .child(canvas_pane(cfg().shapes, 1000.0, 600.0, true, true))
            .into_element(),
        "core" => {
            at_measure_start(spawn_core_threads);
            view()
                .flex_1()
                .bg(col(BG))
                .p(px(10.0))
                .child(canvas_pane(1000, 1000.0, 600.0, false, false))
                .into_element()
        }
        "stress" => {
            at_measure_start(spawn_core_threads);
            view()
                .flex_1()
                .flex_row()
                .bg(col(BG))
                .child(view().flex_1().child(chrome(list_pane(true, true), rows)))
                .child(
                    view()
                        .w(px(400.0))
                        .h(px(720.0))
                        .flex_shrink_0()
                        .child(canvas_pane(cfg().shapes, 400.0, 620.0, true, true)),
                )
                .into_element()
        }
        "text" => chrome(text_pane(), "text1mb.txt (1 MB)".to_owned()).into_element(),
        "images" => chrome(images_pane(), "600 images".to_owned()).into_element(),
        "nav" => nav_pane().into_element(),
        _ => view().into_element(),
    };
    view().flex_1().child(root)
}

// ---------------------------------------------------------------- list ----

fn pastille(i: usize) -> Hsla {
    let f = fnv(i as u32);
    let (r, g, b) = (f & 255, (f >> 8) & 255, (f >> 16) & 255);
    rgb((r << 16) | (g << 8) | b).into()
}

fn quote(i: usize) -> String {
    let p = (fnv(i as u32 + 7) % 100_000) as f64 / 100.0;
    format!("Quote {i} \u{2014} {p:.2}")
}

/// Scripted velocity (px/s) at time t of the measured window: 4 s at 3000 px/s
/// down, then alternating flings v0 = 20000 px/s, v(t) = v0 exp(-2t), 1.5 s each.
fn script_v(t: f64) -> f64 {
    if t < 4.0 {
        3000.0
    } else {
        let k = ((t - 4.0) / 1.5).floor();
        let tau = t - 4.0 - k * 1.5;
        let s = if (k as i64) % 2 == 0 { 1.0 } else { -1.0 };
        s * 20000.0 * (-2.0 * tau).exp()
    }
}

/// A virtualized, recycled list of 100 000 rows of 48..128 pt.
/// `scripted`: the scripted scroll runs; `stream`: prices update.
fn list_pane(scripted: bool, stream: bool) -> impl IntoElement {
    // Price per row, in cents (price = cents / 100.0); initial = the subtitle's quote.
    let prices: Rc<RefCell<Vec<u32>>> =
        Rc::new(RefCell::new((0..100_000u32).map(|i| fnv(i + 7) % 100_000).collect()));
    let version = signal(0u64);
    let list = element_ref();
    let limit = Rc::new(std::cell::Cell::new(f32::MAX));

    if scripted {
        let list = list.clone();
        let prices = prices.clone();
        let limit = limit.clone();
        let rate = if cfg().scenario == "stress" { 5000 } else { cfg().rate };
        std::mem::forget(spawn(async move {
            let mut started = false;
            let mut last_t = 0.0f64;
            let mut pos = 0.0f64;
            loop {
                next_frame().await;
                let Some(t) = measured_t() else { continue };
                if !started {
                    started = true;
                    last_t = t;
                    if stream {
                        spawn_producer(rate);
                    }
                }
                // Integrate the scripted velocity over the real frame interval (midpoint).
                let dt = t - last_t;
                last_t = t;
                if dt > 0.0 {
                    let v = script_v(t - dt / 2.0);
                    pos = (pos + v * dt).clamp(0.0, limit.get().max(0.0) as f64);
                    list.scroll_to_offset(px(pos as f32), false);
                }
                if stream {
                    // Apply the queue's batch once per frame.
                    let batch = std::mem::take(&mut *QUEUE.lock().unwrap());
                    if !batch.is_empty() {
                        let mut p = prices.borrow_mut();
                        for (i, c) in &batch {
                            p[*i as usize] = *c;
                        }
                        APPLIED.fetch_add(batch.len() as u64, Relaxed);
                        drop(p);
                        version.update(|v| *v += 1);
                    }
                }
            }
        }));
    }

    let row_prices = prices.clone();
    // Docs (Lists): rows not measured yet count at the estimated size; the exact mean
    // row height of the data model is the closest estimate there is.
    let mean_h = sum_heights() as f32 / 100_000.0;
    virtual_list(100_000usize, move |at| {
        let prices = row_prices.clone();
        view()
            .h(move || px(row_h(at.index() as u32) as f32))
            .flex_row()
            .items_center()
            .px(px(14.0))
            .gap(px(14.0))
            .overflow_hidden()
            .bg(move || if at.index() % 2 == 0 { col(ROW_A) } else { col(ROW_B) })
            .child(
                view()
                    .w(px(32.0))
                    .h(px(32.0))
                    .rounded(px(16.0))
                    .flex_shrink_0()
                    .bg(move || pastille(at.index())),
            )
            .child(
                view()
                    .flex_1()
                    .h(px(40.0))
                    .overflow_hidden()
                    .child(
                        text(move || {
                            ROWS_BOUND.fetch_add(1, Relaxed);
                            format!("Item #{}", at.index())
                        })
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(col(TEXT))
                        .number_of_lines(1),
                    )
                    .child(
                        text(move || quote(at.index()))
                            .text_size(px(12.0))
                            .text_color(col(MUTED))
                            .number_of_lines(1),
                    ),
            )
            .child(
                text(move || {
                    version.get();
                    let c = prices.borrow()[at.index()];
                    format!("{:.2}", c as f64 / 100.0)
                })
                .w(px(110.0))
                .flex_shrink_0()
                .text_right()
                .tabular_nums()
                .text_size(px(15.0))
                .text_color(col(TEXT)),
            )
    })
    .estimated_item_size(px(mean_h))
    .on_scroll(move |e: &ScrollEvent| {
        f64_store(&FINAL_OFFSET, e.offset.y.0 as f64);
        f64_store(&CONTENT_H, e.content_size.height.0 as f64);
        limit.set((e.content_size.height.0 - e.size.height.0).max(0.0));
    })
    .flex_1()
    .element_ref(&list)
}

// -------------------------------------------------------------- canvas ----

/// Bounce on one axis as an animated node: triangle wave of (|v| t + phase) with
/// period 2L, i.e. `((t*|v| + s) mod 2L)` interpolated through [0, L, 2L] -> [0, L, 0].
fn bounce(clock: &AnimatedValue, x0: f32, v: f32, l: f32) -> AnimatedNode {
    let l = l.max(1.0);
    let s = if v >= 0.0 { x0 } else { 2.0 * l - x0 };
    clock
        .mul(v.abs())
        .add(s)
        .modulo(2.0 * l)
        .interpolate(&[0.0, l, 2.0 * l], &[0.0, l, 0.0])
}

/// Per-shape constants of the data model.
struct ShapeDef {
    size: f32,
    x0: f32,
    y0: f32,
    vx: f32,
    vy: f32,
    c1: Hsla,
    c2: Hsla,
}

fn shape_def(i: usize, w: f32, h: f32) -> ShapeDef {
    let f = fnv(i as u32);
    let g = fnv(i as u32 + 1_000_003);
    let size = 16.0 + (f % 25) as f32;
    let ang = (g % 360) as f32 * std::f32::consts::PI / 180.0;
    let speed = 60.0 + (f % 140) as f32;
    ShapeDef {
        size,
        x0: (fnv(i as u32 + 77) as f32 / u32::MAX as f32) * (w - size),
        y0: (fnv(i as u32 + 78) as f32 / u32::MAX as f32) * (h - size),
        vx: ang.cos() * speed,
        vy: ang.sin() * speed,
        c1: col(f & 0xffffff),
        c2: col(g & 0xffffff),
    }
}

/// The same triangle wave as [`bounce`], evaluated in Rust (diagnostic re-record path).
fn bounce_at(t: f32, x0: f32, v: f32, l: f32) -> f32 {
    let l = l.max(1.0);
    let s = if v >= 0.0 { x0 } else { 2.0 * l - x0 };
    let u = (t * v.abs() + s) % (2.0 * l);
    if u < l { u } else { 2.0 * l - u }
}

/// Records the shapes. `pos(i)` translates to shape i (animated nodes or numbers).
fn record_shapes(
    defs: &[ShapeDef],
    w: f32,
    h: f32,
    rich: bool,
    grads: &[Arc<CanvasGradient>],
    pos: &dyn Fn(Painter, usize) -> Painter,
) -> Painter {
    let shadow_clip = diag("SHADOW_CLIP");
    let no_shadow = diag("NOSHADOW");
    let shadow_color: Hsla = col(0x000000).opacity(0.35);
    let mut p = Painter::new()
        .save()
        .begin_path()
        .round_rect(Bounds::new(pt(0.0, 0.0), Size::new(px(w), px(h))), px(16.0))
        .clip();
    for (i, d) in defs.iter().enumerate() {
        let size = d.size;
        let shadow = rich && i % 10 == 0 && !no_shadow;
        p = pos(p.save(), i);
        if shadow && shadow_clip {
            // Diagnostic only (BENCH_SHADOW_CLIP=1): bounds NSH's per-shadow mask, which is as
            // large as the current clip, to the shape's box + blur. Not the documented usage.
            p = p
                .begin_path()
                .rect(Bounds::new(pt(-16.0, -16.0), Size::new(px(size + 32.0), px(size + 32.0))))
                .clip();
        }
        if rich {
            if shadow {
                p = p.shadow(shadow_color, px(8.0), pt(2.0, 2.0));
            }
            p = p.fill_style(grads[i].clone());
        } else {
            p = p.fill_color(d.c1);
        }
        p = p.begin_path();
        let b = Bounds::new(pt(0.0, 0.0), Size::new(px(size), px(size)));
        p = match i % 3 {
            0 => p.rect(b),
            1 => p.arc(size / 2.0, size / 2.0, size / 2.0, 0.0, std::f32::consts::TAU),
            _ => p.round_rect(b, px(size / 4.0)),
        };
        p = p.fill().restore();
    }
    p.restore()
}

fn canvas_pane(n: usize, w: f32, h: f32, rich: bool, graph: bool) -> impl IntoElement {
    let defs: Rc<Vec<ShapeDef>> = Rc::new((0..n).map(|i| shape_def(i, w, h)).collect());
    // Gradients in each shape's local coordinates, made once and shared by every recording.
    let grads: Rc<Vec<Arc<CanvasGradient>>> = Rc::new(
        defs.iter()
            .map(|d| {
                Arc::new(
                    CanvasGradient::linear(pt(0.0, 0.0), pt(d.size, d.size))
                        .add_color_stop(Percentage(0.0), d.c1)
                        .add_color_stop(Percentage(1.0), d.c2),
                )
            })
            .collect(),
    );
    let main = if !diag("ANIMATED") {
        // Default: the documented "redraw from signals" pattern (docs: Canvas element, When it
        // redraws): positions computed on the engine thread, the drawing re-recorded every engine
        // frame. The alternative the Performance page suggests (animated values, BENCH_ANIMATED=1
        // below) was measured slower on NSH 0.2.0 / Linux: same replay cost, plus a multi-second
        // stall building 2 x N animated nodes, and slower per frame at 1 000 shapes.
        let tick = signal(0u64);
        std::mem::forget(spawn(async move {
            loop {
                next_frame().await;
                tick.update(|t| *t += 1);
            }
        }));
        let t0 = std::time::Instant::now();
        canvas(move || {
            tick.get();
            let t = t0.elapsed().as_secs_f32();
            let d2 = defs.clone();
            record_shapes(&defs, w, h, rich, &grads, &move |p, i| {
                let d = &d2[i];
                p.translate(bounce_at(t, d.x0, d.vx, w - d.size), bounce_at(t, d.y0, d.vy, h - d.size))
            })
        })
    } else {
        // Diagnostic (BENCH_ANIMATED=1): positions as native animated values; the drawing is
        // recorded once and the GTK thread redraws it as the clock (seconds) advances.
        let clock = animated_value(0.0);
        clock.timing(100_000.0, 100_000_000.0, Easing::Linear);
        let nodes: Vec<(AnimatedNode, AnimatedNode)> = defs
            .iter()
            .map(|d| (bounce(&clock, d.x0, d.vx, w - d.size), bounce(&clock, d.y0, d.vy, h - d.size)))
            .collect();
        let drawing = record_shapes(&defs, w, h, rich, &grads, &|p, i| p.translate(&nodes[i].0, &nodes[i].1));
        canvas(drawing)
    };
    let main = main.w(px(w)).h(px(h)).flex_shrink_0();
    let mut col_view = view().child(main);
    if graph {
        // The polyline gains one point per engine frame: its data changes every frame, so
        // it is a canvas of its own (docs: Canvas, Performance: split what changes often).
        let tick = signal(0u64);
        std::mem::forget(spawn(async move {
            loop {
                next_frame().await;
                tick.update(|t| *t += 1);
            }
        }));
        let ring: Rc<RefCell<std::collections::VecDeque<f32>>> =
            Rc::new(RefCell::new((0..10_000).map(wave).collect()));
        let phase = Rc::new(std::cell::Cell::new(10_000u32));
        let gw = w;
        col_view = col_view.child(
            canvas(move || {
                tick.get();
                let mut r = ring.borrow_mut();
                r.pop_front();
                let k = phase.get();
                phase.set(k + 1);
                r.push_back(wave(k));
                let mut p = Painter::new()
                    .fill_color(col(0xffffff))
                    .fill_rect(0.0, 0.0, gw, 100.0)
                    .stroke_color(col(0x2f6fed))
                    .line_width(1.0)
                    .begin_path();
                let n = r.len() as f32;
                for (i, v) in r.iter().enumerate() {
                    let x = i as f32 / (n - 1.0) * gw;
                    let y = 50.0 - v * 44.0;
                    p = if i == 0 { p.move_to(x, y) } else { p.line_to(x, y) };
                }
                p.stroke()
            })
            .w(px(w))
            .h(px(100.0))
            .flex_shrink_0(),
        );
    }
    col_view
}

/// Diagnostic knobs (BENCH_<name>=1), off by default.
fn diag(name: &str) -> bool {
    std::env::var_os(format!("BENCH_{name}")).is_some()
}

fn wave(i: u32) -> f32 {
    let t = i as f32 * 0.01;
    (t.sin() * 0.6 + (t * 3.7).sin() * 0.25 + ((fnv(i) % 1000) as f32 / 1000.0 - 0.5) * 0.15).clamp(-1.0, 1.0)
}

// ---------------------------------------------------------------- text ----

/// 1 MB of text: one selectable `text` in a native scroll view (docs: Text, selection;
/// Scrolling, scrolling from code). NSH 0.2.0 has no long-text component (the Editor came
/// after the release) and no programmatic text selection.
fn text_pane() -> impl IntoElement {
    let path = format!("{}/text1mb.txt", cfg().assets);
    let content: SharedString = std::fs::read_to_string(&path).unwrap_or_default().into();
    TEXT_BYTES.store(content.len() as u64, Relaxed);
    let made = std::time::Instant::now();
    let height = Rc::new(std::cell::Cell::new(0.0f32));
    let viewport = Rc::new(std::cell::Cell::new(0.0f32));
    let sc = element_ref();
    {
        let sc = sc.clone();
        let height = height.clone();
        let viewport = viewport.clone();
        std::mem::forget(spawn(async move {
            loop {
                next_frame().await;
                let Some(t) = measured_t() else { continue };
                let m = (height.get() - viewport.get()).max(1.0);
                // 3000 px/s back and forth between the top and the end.
                let d = (t as f32 * 3000.0) % (2.0 * m);
                let y = if d < m { d } else { 2.0 * m - d };
                sc.scroll_to(pt(0.0, y), false);
            }
        }));
    }
    let h2 = height.clone();
    scroll()
        .flex_1()
        .bg(col(0xffffff))
        .on_layout(move |b: &Bounds<Points>| viewport.set(b.size.height.0))
        .on_scroll(move |e: &ScrollEvent| f64_store(&CONTENT_H, e.content_size.height.0 as f64))
        .child(
            text(content)
                .selectable(true)
                .text_size(px(14.0))
                .text_color(col(TEXT))
                .p(px(16.0))
                .on_layout(move |b: &Bounds<Points>| {
                    if TEXT_LAID_MS.load(Relaxed) == 0 {
                        // From building the element to its first layout (Pango, engine thread).
                        f64_store(&TEXT_LAYOUT_MS, made.elapsed().as_secs_f64() * 1000.0);
                        TEXT_LAID_MS.store(now_ns() / 1_000_000 + 1, Relaxed);
                    }
                    h2.set(b.size.height.0);
                }),
        )
        .element_ref(&sc)
}

// -------------------------------------------------------------- images ----

fn images_pane() -> impl IntoElement {
    let list = element_ref();
    let zoom = animated_value(1.0);
    let pan = animated_value(0.0);
    let list2 = list.clone();
    std::mem::forget(spawn(async move {
        let mut started = false;
        let limit = 60.0 * 128.0 - 656.0;
        loop {
            next_frame().await;
            let Some(t) = measured_t() else { continue };
            if !started {
                started = true;
                // Zoom 1 -> 3 -> 1 over 6 s with a pan, played natively (animated values).
                let z = zoom.clone();
                let p = pan.clone();
                std::mem::forget(spawn(async move {
                    loop {
                        p.timing(-200.0, 3000.0, Easing::Linear);
                        z.timing(3.0, 3000.0, Easing::Linear).await;
                        p.timing(0.0, 3000.0, Easing::Linear);
                        z.timing(1.0, 3000.0, Easing::Linear).await;
                        if STOP.load(Relaxed) {
                            break;
                        }
                    }
                }));
            }
            // 3000 px/s down then up (triangle wave over the grid).
            let d = (t as f32 * 3000.0) % (2.0 * limit);
            let y = if d < limit { d } else { 2.0 * limit - d };
            list2.scroll_to_offset(px(y), false);
        }
    }));
    view().flex_1().overflow_hidden().child(
        view().flex_1().bind_scale(&zoom).bind_translate_x(&pan).child(
            virtual_list(600usize, move |at| {
                let started = Rc::new(std::cell::Cell::new(None::<std::time::Instant>));
                let s1 = started.clone();
                let s2 = started.clone();
                image(move || format!("asset:/images/img{:03}.png", at.index()))
                    .w(px(128.0))
                    .h(px(128.0))
                    .resize_mode(ResizeMode::Cover)
                    .fade_duration(0.0)
                    .on_load_start(move || s1.set(Some(std::time::Instant::now())))
                    .on_load_end(move || {
                        // load start -> load end: queue + decode off the main thread + texture.
                        if let Some(t0) = s2.take() {
                            IMAGES_DECODED.fetch_add(1, Relaxed);
                            DECODE_US.fetch_add(t0.elapsed().as_micros() as u64, Relaxed);
                            let i = at.index();
                            IMAGES_SEEN.lock().unwrap()[i / 64] |= 1 << (i % 64);
                        }
                    })
            })
            .columns(10)
            .estimated_item_size(px(128.0))
            .flex_1()
            .element_ref(&list),
        ),
    )
}

// ----------------------------------------------------------------- nav ----

fn screen_view(n: u32, color: u32) -> impl IntoElement {
    view()
        .flex_1()
        .bg(col(color))
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .child(text(format!("Screen {n}")).text_size(px(32.0)).text_color(col(0xffffff)))
        .child(view().w(px(300.0)).h(px(200.0)).rounded(px(16.0)).bg(col(0xffffff)).opacity(0.25))
}

fn nav_pane() -> impl IntoElement {
    let routes = signal(Vec::<u32>::new());
    let modal = signal(false);
    at_measure_start(move || {
        // A background thread keeps about one core busy during the run.
        std::thread::Builder::new()
            .name("burner".into())
            .spawn(|| {
                let mut x: u64 = 1;
                while !STOP.load(Relaxed) {
                    for _ in 0..100_000 {
                        x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407));
                    }
                }
                CORE_SINK.fetch_add(x, Relaxed);
            })
            .unwrap();
        std::mem::forget(spawn(async move {
            let step = |ms: u64| timer(Duration::from_millis(ms));
            // 5 cycles: push, push, modal in/out (fade 200 ms), pop, pop = 20 slides of 300 ms.
            for _cycle in 0..5 {
                routes.update(|r| r.push(1));
                step(350).await;
                TRANSITIONS.fetch_add(1, Relaxed);
                routes.update(|r| r.push(2));
                step(350).await;
                TRANSITIONS.fetch_add(1, Relaxed);
                modal.set(true);
                step(250).await;
                modal.set(false);
                step(250).await;
                MODALS.fetch_add(1, Relaxed);
                routes.update(|r| {
                    r.pop();
                });
                step(350).await;
                TRANSITIONS.fetch_add(1, Relaxed);
                routes.update(|r| {
                    r.pop();
                });
                step(350).await;
                TRANSITIONS.fetch_add(1, Relaxed);
            }
        }));
    });
    view()
        .flex_1()
        .child(
            nsh_base::nav_stack()
                .flex_1()
                .child(nsh_base::nav_page().title("A").header(view().h(px(0.0))).child(screen_view(0, 0x2f6fed)))
                .child(each(
                    move || routes.get(),
                    |id: &u32| *id,
                    move |id, _depth| {
                        let n = id.get_untracked();
                        nsh_base::nav_page()
                            .title(format!("S{n}"))
                            .header(view().h(px(0.0)))
                            .child(screen_view(n, if n == 1 { 0xd9480f } else { 0x2b8a3e }))
                    },
                )),
        )
        .child(when(
            move || modal.get(),
            move || {
                nsh_base::dialog().animation(ModalAnimation::Fade).child(
                    view()
                        .flex_1()
                        .bg(col(0x000000).opacity(0.45))
                        .items_center()
                        .justify_center()
                        .child(view().w(px(420.0)).h(px(260.0)).rounded(px(16.0)).bg(col(0xffffff))),
                )
            },
        ))
}
