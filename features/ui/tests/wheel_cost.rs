//! What a wheel notch costs the chart's process (FastTrackStudios/
//! architect#839: on a 120 Hz display the wheel dropped a third of its
//! frames while a lasso held 120 fps). A notch is the view's input, the
//! frame of layers it then records, and the batch those layers send. A
//! scroll is a new transform on layers already recorded: nothing of the
//! chart is copied or drawn again for it.
//!
//! Timed on this thread's CPU clock (`/proc/thread-self/schedstat`), over
//! many notches, so a loaded machine's waits do not count; elsewhere the
//! clock reads zero and only the bytes are checked. `--nocapture` prints
//! the means.

#![cfg(feature = "paint-view")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::time::{Duration, Instant};

use architect::action::{ActionCaller, ActionList};
use architect_scene::ops::Sender;
use architect_workspace::paint::RegionEvent;
use architect_workspace::{
    FrameClock, Geometry, HostData, KindId, PaneId, Placement, ViewCtx, ViewHost, ViewInput,
};
use keyflow::engraver::renderer::ChartTheme;
use keyflow_ui::paint_view::{ChartSnapshot, ChartView, chart_mount};

const NOTCHES: u32 = 600;

fn chart_text(name: &str) -> String {
    let path = format!(
        "{}/../../apps/architect/vault/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// This thread's time on a CPU: what the machine's load does not stretch,
/// as a wall clock does while the thread waits its turn. Zero off Linux.
fn cpu() -> Duration {
    std::fs::read_to_string("/proc/thread-self/schedstat")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse::<u64>().ok())
        .map_or(Duration::ZERO, Duration::from_nanos)
}

#[test]
fn a_wheel_notch_is_a_transform_on_layers_already_recorded() {
    let text = chart_text("JT Recital/02 Drive To LA.kf");
    let data = HostData::new(ChartSnapshot {
        chart: keyflow::parse(&text).unwrap(),
        revision: 1,
        playhead_secs: None,
        theme: Some(ChartTheme::default()),
        // Zoomed in, so a vertical wheel scrolls.
        view: ChartView {
            zoom: Some(2.0),
            scroll: (0.0, 200.0),
        },
        input: None,
    });
    let mount = chart_mount(data.clone());
    let ctx = ViewCtx::new(
        PaneId::new("chart"),
        KindId::paint(),
        (ViewHost::Blitz, Placement::Pane),
        (ActionList::new(), ActionCaller::app()),
        FrameClock::new(Duration::from_millis(4), || Duration::ZERO),
    )
    .with_extras(mount.mount.host_extras())
    .with_geometry(Geometry {
        width: 1280.0,
        height: 820.0,
        scale: 2.0,
    });
    let mut view = mount.mount.make_view(&ctx);
    let mut sender = Sender::default();
    let deadline = Instant::now() + Duration::from_secs(60);
    let layers = loop {
        let layers = view.layers(&ctx).unwrap();
        if !view.animating(&ctx) && !layers.is_empty() {
            break layers;
        }
        assert!(Instant::now() < deadline, "the chart never laid out");
        std::thread::sleep(Duration::from_millis(5));
    };
    let _ = sender.frame(&layers);

    let (mut input, mut frame, mut send, mut bytes) =
        (Duration::ZERO, Duration::ZERO, Duration::ZERO, 0usize);
    for notch in 0..NOTCHES {
        // Down and back, over the middle of the pane; the host's regions
        // move under the still pointer, so it enters another element now
        // and then.
        let dy = if (notch / 30) % 2 == 0 { 40.0 } else { -40.0 };
        let started = cpu();
        view.input(
            &ctx,
            &ViewInput::Wheel {
                dx: 0.0,
                dy,
                x: 640.0,
                y: 410.0,
                mods: vec![],
            },
        );
        if notch % 5 == 0 {
            view.region(&ctx, &RegionEvent::Enter(u64::from(notch % 40)));
        }
        let layered = cpu();
        let layers = view.layers(&ctx).unwrap();
        let sent = cpu();
        bytes += sender.frame(&layers).unwrap_or_default().len();
        let done = cpu();
        input += layered - started;
        frame += sent - layered;
        send += done - sent;
    }
    let mean = |d: Duration| d / NOTCHES;
    println!(
        "a wheel notch, mean CPU: input {:?}, layers {:?}, send {:?}; {} bytes",
        mean(input),
        mean(frame),
        mean(send),
        bytes / NOTCHES as usize,
    );
    // A transform a page, never its content.
    assert!(
        bytes / (NOTCHES as usize) < 400,
        "a notch sent {} bytes",
        bytes / NOTCHES as usize
    );
    // Recording nothing again, the view's part of a notch is tens of
    // microseconds, unoptimised (it was about 500 while every frame copied
    // the chart; the bound has room for a slower machine, not for that).
    assert!(
        mean(input + frame) < Duration::from_micros(200),
        "the view spent {:?} a notch",
        mean(input + frame)
    );
}
