//! The chart's pane in a browser.
//!
//! On a desk the chart is a Blitz widget (`linked::PaintPane`); a browser
//! has no Blitz, so the same paint view, made by the same [`PaintMount`]
//! (`keyflow_ui::paint_view::chart_mount_with`), is drawn on a WebGL2
//! canvas by Architect's `WebPaintPane`. Its host data (the chart, its
//! selection and status) is the mount's, so every action that changes
//! the chart changes it here too; their wakes reach the canvas through
//! [`PaintMount::wake_handle`], looked at once a frame.
//!
//! The playhead: on a desk a chart on a session follows the session's
//! transport (`session_view`); in a browser the engine runs in the page
//! and moves the app's one playhead, so a chart follows that: while the
//! playing item is this chart's song (its recording or its note in the
//! chart's folder), the chart's cursor is at the item's time.

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use architect::action::{ActionCaller, ActionList};
use architect_workspace::paint::web_ui::{WebPaintMount, WebPaintPane};
use architect_workspace::{
    FrameClock, KindId, PaintMount, Painter, PaneId, Placement, ViewCtx, ViewHost,
};
use dioxus::prelude::*;

/// How often the mount's wake and the playhead are looked at: a frame.
const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

/// The chart's paint view on a canvas: `mount` made once, in pane `pane`.
#[component]
pub fn WebChartPane(mount: PaintMount, pane: String, path: String) -> Element {
    let actions = try_use_context::<Arc<ActionList>>();
    let theme = architect_workspace::paint::pane_tokens();
    let web = use_hook(|| {
        let mount = mount.clone();
        let actions = actions.map(|a| (*a).clone()).unwrap_or_default();
        let pane = PaneId::new(pane.clone());
        WebPaintMount::new(move || {
            let clock = FrameClock::new(
                mount.frame_budget(),
                architect_workspace::paint::anim::wall_elapsed,
            );
            let ctx = ViewCtx::new(
                pane.clone(),
                KindId::paint(),
                (ViewHost::Web, Placement::Pane),
                (actions.clone(), ActionCaller::app()),
                clock,
            )
            .with_extras(mount.host_extras());
            let view = mount.make_view(&ctx);
            Painter::boxed(ctx, view).with_stats(mount.frame_stats())
        })
    });
    // The mount's wakes (an edit, a reread, the theme) and the playhead,
    // once a frame, onto the canvas.
    {
        let (wake, web_wake) = (mount.wake_handle(), web.wake_handle());
        let path = Rc::new(path.clone());
        use_future(move || {
            let (wake, web_wake, path) = (wake.clone(), web_wake.clone(), Rc::clone(&path));
            async move {
                let mut followed = None::<u64>;
                loop {
                    gloo_timers::future::sleep(FRAME).await;
                    let at = playhead_for(&path);
                    let bits = at.map(|s| (s * 1000.0).round().to_bits());
                    if bits != followed {
                        followed = bits;
                        crate::set_playhead(&path, at);
                    }
                    if wake.take() {
                        web_wake.wake();
                    }
                }
            }
        });
    }
    rsx! { WebPaintPane { mount: web, theme } }
}

/// The time of the app's playhead, seconds, while the item it plays is
/// the song of the chart at `chart`: an item whose recording (`media`) or
/// id (a song's note) is in the chart's song folder.
fn playhead_for(chart: &str) -> Option<f64> {
    let song = crate::song_folder(chart)?;
    let now = architect_playhead::now_ms();
    architect_playhead::with(|deck| {
        let source = deck.current()?;
        let item = source.item()?;
        let ours = [item.media.as_deref(), Some(item.id.as_str())]
            .into_iter()
            .flatten()
            .any(|p| Path::new(p).starts_with(&song));
        ours.then(|| source.time(now))
    })
}
