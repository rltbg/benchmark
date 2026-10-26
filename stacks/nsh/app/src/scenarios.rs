//! The reference app's Rust components (they run on NSH's engine thread).

use std::{cell::RefCell, rc::Rc, sync::Arc, sync::atomic::Ordering::Relaxed, time::Duration};

use nsh_core::{
    Bounds, Point, Points, Size,
    canvas::CanvasGradient,
    color::{Rgba, rgb},
    ui::{ScrollEvent, prelude::*},
    ModalAnimation,
};

use crate::shared::*;

pub fn unsupported(_scenario: &str) -> Option<&'static str> {
    None
}

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

/// The scenario root.
#[component]
pub fn Bench() -> impl IntoElement {
    let sc = cfg().scenario.clone();
    let rows = "100000 rows".to_owned();
    let root: AnyElement = match sc.as_str() {
        "idle" => chrome(list_pane(false, false), rows).into_element(),
        "list" => chrome(list_pane(true, false), rows).into_element(),
        "stream" => chrome(list_pane(true, true), rows).into_element(),
        "canvas" => view()
            .flex_1()
            .bg(col(BG))
            .child(canvas_pane(cfg().shapes, 1000.0, 600.0, true, true))
            .into_element(),
        "core" => {
            spawn_core_threads();
            view()
                .flex_1()
                .bg(col(BG))
                .child(canvas_pane(1000, 1000.0, 600.0, false, false))
                .into_element()
        }
        "stress" => {
            spawn_core_threads();
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
        "text" => chrome(text_pane(), "1 MB text".to_owned()).into_element(),
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

/// A virtualized, recycled list of 100 000 rows of 48..128 pt.
/// `scripted`: the scripted scroll runs; `stream`: prices update.
fn list_pane(scripted: bool, stream: bool) -> impl IntoElement {
    let prices: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(
        (0..100_000u32).map(|i| fnv(i + 7) % 100_000).collect(),
    ));
    let version = signal(0u64);
    let list = element_ref();
    let limit = Rc::new(std::cell::Cell::new(f32::MAX));
    let offset = Rc::new(std::cell::Cell::new(0.0f32));

    if scripted {
        let list = list.clone();
        let prices = prices.clone();
        let limit = limit.clone();
        let offset = offset.clone();
        let rate = if cfg().scenario == "stress" { 5000 } else { cfg().rate };
        std::mem::forget(spawn(async move {
            let mut started = false;
            let mut last_t = 0.0f64;
            let mut pos = 0.0f32;
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
                // Scripted velocity: 4 s at 3000 px/s, then alternating flings.
                let dt = t - last_t;
                last_t = t;
                if dt > 0.0 {
                    let mid = t - dt / 2.0;
                    let v = if mid < 4.0 {
                        3000.0
                    } else {
                        let k = ((mid - 4.0) / 1.5).floor();
                        let tau = mid - 4.0 - k * 1.5;
                        let s = if (k as i64) % 2 == 0 { 1.0 } else { -1.0 };
                        s * 20000.0 * (-2.0 * tau).exp()
                    };
                    pos = (pos + (v * dt) as f32).clamp(0.0, limit.get().max(0.0));
                    list.scroll_to_offset(px(pos), false);
                    offset.set(pos);
                }
                if stream {
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
                        .text_color(col(TEXT)),
                    )
                    .child(
                        text(move || {
                            let q = quote(at.index());
                            format!("{q}  {q}  {q}")
                        })
                        .text_size(px(12.0))
                        .text_color(col(MUTED)),
                    ),
            )
            .child(
                text(move || {
                    version.get();
                    let c = prices.borrow()[at.index()];
                    format!("{:.2}", c as f64 / 100.0)
                })
                .w(px(90.0))
                .text_right()
                .tabular_nums()
                .text_size(px(15.0))
                .text_color(col(TEXT)),
            )
    })
    .estimated_item_size(px(88.0))
    .on_scroll(move |e: &ScrollEvent| {
        f64_store(&FINAL_OFFSET, e.offset.y.0 as f64);
        f64_store(&CONTENT_H, e.content_size.height.0 as f64);
        limit.set((e.content_size.height.0 - e.size.height.0).max(0.0));
    })
    .flex_1()
    .element_ref(&list)
}

// -------------------------------------------------------------- canvas ----

#[derive(Clone)]
struct Shape {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    w: f32,
    h: f32,
    kind: u8,
    shadow: bool,
    grad: Arc<CanvasGradient>,
    flat: Hsla,
}

fn make_shapes(n: usize, w: f32, h: f32) -> Vec<Shape> {
    (0..n)
        .map(|i| {
            let f = fnv(i as u32);
            let g = fnv(i as u32 + 1_000_003);
            let size = 16.0 + (f % 25) as f32;
            let c1 = col(f & 0xffffff);
            let c2 = col(g & 0xffffff);
            let grad = CanvasGradient::linear(pt(0.0, 0.0), pt(size, size))
                .add_color_stop(Percentage(0.0), c1)
                .add_color_stop(Percentage(1.0), c2);
            let ang = (g % 360) as f32 * std::f32::consts::PI / 180.0;
            let speed = 60.0 + (f % 140) as f32;
            Shape {
                x: (fnv(i as u32 + 77) as f32 / u32::MAX as f32) * (w - size),
                y: (fnv(i as u32 + 78) as f32 / u32::MAX as f32) * (h - size),
                vx: ang.cos() * speed,
                vy: ang.sin() * speed,
                w: size,
                h: size,
                kind: (i % 3) as u8,
                shadow: i % 10 == 0,
                grad: Arc::new(grad),
                flat: c1,
            }
        })
        .collect()
}

fn step_shapes(shapes: &mut [Shape], dt: f32, w: f32, h: f32) {
    for s in shapes {
        s.x += s.vx * dt;
        s.y += s.vy * dt;
        if s.x < 0.0 {
            s.x = -s.x;
            s.vx = -s.vx;
        } else if s.x > w - s.w {
            s.x = 2.0 * (w - s.w) - s.x;
            s.vx = -s.vx;
        }
        if s.y < 0.0 {
            s.y = -s.y;
            s.vy = -s.vy;
        } else if s.y > h - s.h {
            s.y = 2.0 * (h - s.h) - s.y;
            s.vy = -s.vy;
        }
    }
}

fn draw_shapes(shapes: &[Shape], w: f32, h: f32, rich: bool) -> Painter {
    let mut p = Painter::new()
        .save()
        .begin_path()
        .round_rect(Bounds::new(pt(0.0, 0.0), Size::new(px(w), px(h))), px(16.0))
        .clip();
    let naive_shadow = diag("NAIVE_SHADOW");
    let shadow_color = rgb(0x000000).into_hsla_alpha(0.35);
    for s in shapes {
        p = p.save();
        if rich && s.shadow && !diag("NOSHADOW") && !naive_shadow {
            // NSH's Linux canvas allocates and blurs a shadow mask as large as
            // the current clip's extents for every shadowed draw; clipping to
            // the shape's box (+ blur spread) first keeps that mask small.
            p = p
                .begin_path()
                .rect(Bounds::new(pt(s.x - 16.0, s.y - 16.0), Size::new(px(s.w + 32.0), px(s.h + 32.0))))
                .clip();
        }
        p = p.translate(s.x, s.y);
        if rich {
            if s.shadow && !diag("NOSHADOW") {
                p = p.shadow(shadow_color, px(8.0), pt(2.0, 2.0));
            }
            if diag("FLAT") { p = p.fill_color(s.flat); } else { p = p.fill_style(s.grad.clone()); }
        } else {
            p = p.fill_color(s.flat);
        }
        p = p.begin_path();
        let b = Bounds::new(pt(0.0, 0.0), Size::new(px(s.w), px(s.h)));
        p = match s.kind {
            0 => p.rect(b),
            1 => p.arc(s.w / 2.0, s.h / 2.0, s.w / 2.0, 0.0, std::f32::consts::TAU),
            _ => p.round_rect(b, px(s.w / 4.0)),
        };
        p = p.fill().restore();
    }
    p.restore()
}

trait HslaAlpha {
    fn into_hsla_alpha(self, a: f32) -> Hsla;
}
impl HslaAlpha for Rgba {
    fn into_hsla_alpha(self, a: f32) -> Hsla {
        Hsla::from(self).opacity(a)
    }
}

fn canvas_pane(n: usize, w: f32, h: f32, rich: bool, graph: bool) -> impl IntoElement {
    let tick = signal(0u64);
    let shapes = Rc::new(RefCell::new(make_shapes(n, w, h)));
    let last = Rc::new(std::cell::Cell::new(None::<std::time::Instant>));
    std::mem::forget(spawn(async move {
        loop {
            next_frame().await;
            tick.update(|t| *t += 1);
        }
    }));
    let shapes2 = shapes.clone();
    let main = canvas(move || {
        tick.get();
        let t0 = std::time::Instant::now();
        let now = std::time::Instant::now();
        let dt = last.replace(Some(now)).map_or(0.016, |l| (now - l).as_secs_f32()).min(0.1);
        let mut s = shapes2.borrow_mut();
        step_shapes(&mut s, dt, w, h);
        let p = draw_shapes(&s, w, h, rich);
        CANVAS_RECORD_US.fetch_add(t0.elapsed().as_micros() as u64, Relaxed);
        CANVAS_FRAMES.fetch_add(1, Relaxed);
        p
    })
    .w(px(w))
    .h(px(h));
    let mut col_view = view().child(main);
    if graph {
        let ring: Rc<RefCell<std::collections::VecDeque<f32>>> =
            Rc::new(RefCell::new((0..10_000).map(|i| wave(i)).collect()));
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
            .h(px(100.0)),
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

/// 1 MB of text in NSH's `Editor` (read only, soft wrap): the one component of
/// `nsh/base` that virtualizes the rows of a long text and selects in it.
fn text_pane() -> impl IntoElement {
    let path = format!("{}/text1mb.txt", cfg().assets);
    let content: SharedString = std::fs::read_to_string(&path).unwrap_or_default().into();
    let r = component_ref::<nsh_base::Editor>();
    let r2 = r;
    let created = std::time::Instant::now();
    std::mem::forget(spawn(async move {
        let mut max: Option<f32> = None;
        let mut shown = false;
        let mut selected = false;
        let mut last = std::time::Instant::now();
        let mut worst_gap = 0.0f64;
        loop {
            next_frame().await;
            let now = std::time::Instant::now();
            if !shown {
                worst_gap = worst_gap.max((now - last).as_secs_f64() * 1000.0);
                TEXT_LAYOUT_MS.store(worst_gap as u64, Relaxed);
            }
            last = now;
            let Some(api) = r2.api::<nsh_base::EditorApi>() else { continue };
            if !shown {
                let v = api.visible_range();
                if v.end.line > v.start.line {
                    shown = true;
                    TEXT_FIRST_MS.store(now_ns() / 1_000_000, Relaxed);
                }
            }
            let Some(t) = measured_t() else { continue };
            if max.is_none() {
                // The end of the text is where an offset far past it is clamped to.
                api.set_scroll_offset(pt(0.0, 1.0e9));
                max = Some(api.scroll_offset().y.0.max(1.0));
            }
            let m = max.unwrap();
            // 3000 px/s back and forth.
            let d = (t as f32 * 3000.0) % (2.0 * m);
            let y = if d < m { d } else { 2.0 * m - d };
            api.set_scroll_offset(pt(0.0, y));
            if t >= 5.0 && !selected {
                selected = true;
                api.focus();
                api.run(nsh_base::EditorCommand::SelectAll);
            }
            if selected {
                TEXT_SEL.store(api.selected_text().len() as u64, Relaxed);
            }
        }
    }));
    let _ = created;
    nsh_base::editor()
        .value(move || Some(content.clone()))
        .read_only(true)
        .soft_wrap(true)
        .line_numbers(false)
        .flex_1()
        .component_ref(&r)
}

// -------------------------------------------------------------- images ----

fn images_pane() -> impl IntoElement {
    let list = element_ref();
    let zoom = animated_value(1.0);
    let pan = animated_value(0.0);
    let list2 = list.clone();
    std::mem::forget(spawn(async move {
        let mut started = false;
        let mut limit = 0.0f32;
        loop {
            next_frame().await;
            let Some(t) = measured_t() else { continue };
            if !started {
                started = true;
                // Zoom 1 -> 3 -> 1 over 6 s with a pan, played natively.
                let z = zoom.clone();
                let p = pan.clone();
                std::mem::forget(spawn(async move {
                    for _ in 0..1000 {
                        p.timing(-200.0, 3000.0, Easing::Linear);
                        z.timing(3.0, 3000.0, Easing::Linear).await;
                        p.timing(0.0, 3000.0, Easing::Linear);
                        z.timing(1.0, 3000.0, Easing::Linear).await;
                    }
                }));
                limit = 60.0 * 128.0 - 656.0;
            }
            // 3000 px/s down then up (triangle wave over the grid).
            let d = (t as f32 * 3000.0) % (2.0 * limit);
            let y = if d < limit { d } else { 2.0 * limit - d };
            list2.scroll_to_offset(px(y), false);
        }
    }));
    view().flex_1().overflow_hidden().child(
        view()
            .flex_1()
            .bind_scale(&zoom)
            .bind_translate_x(&pan)
            .child(
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
                            // load start -> load end: request, queue, worker decode, texture.
                            if let Some(t0) = s2.take() {
                                IMAGES_DECODED.fetch_add(1, Relaxed);
                                DECODE_US.fetch_add(t0.elapsed().as_micros() as u64, Relaxed);
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
        .child(
            view().w(px(300.0)).h(px(200.0)).rounded(px(16.0)).bg(col(0xffffff)).opacity(0.25),
        )
}

fn nav_pane() -> impl IntoElement {
    let routes = signal(Vec::<u32>::new());
    let modal = signal(false);
    // A background thread keeps about one core busy during the run.
    std::mem::forget(spawn(async move {
        loop {
            next_frame().await;
            if measured_t().is_some() {
                break;
            }
        }
        std::thread::spawn(|| {
            let mut x: u64 = 1;
            while !STOP.load(Relaxed) {
                for _ in 0..100_000 {
                    x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407));
                }
            }
            CORE_SINK.fetch_add(x, Relaxed);
        });
        let step = |ms: u64| timer(Duration::from_millis(ms));
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
    view()
        .flex_1()
        .child(
            nsh_base::nav_stack()
                .flex_1()
                .child(
                    nsh_base::nav_page()
                        .title("A")
                        .header(view().h(px(0.0)))
                        .child(screen_view(0, 0x2f6fed)),
                )
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
