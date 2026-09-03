//! The live chart surface — WebGL2 via `vello_hybrid`, with SVG behind it.
//!
//! # Why this exists when [`crate::chart`] renders SVG
//!
//! Both, on purpose, for two different jobs.
//!
//! SVG is right for a chart that sits there: the guide puts several on a
//! page, they never change once engraved, and the browser scales, prints
//! and selects them for free. A WebGL context is a scarce, stateful
//! resource — browsers cap live ones near sixteen — so one per chart on a
//! page is how the first version of the site wedged its renderer.
//!
//! The editor's preview is the other case, and the one `ChartGraphics`
//! was built for: exactly ONE surface, redrawn as the pointer moves. It
//! wants a live scene it can re-render at pointer speed with a highlight
//! following the beat under the cursor, and re-serialising SVG for that
//! would be absurd.
//!
//! So: one canvas, in one place, and every other chart on the site stays
//! SVG. If that invariant ever slips — a canvas per chart in the guide —
//! the cap comes back.
//!
//! # Falling back
//!
//! WebGL2 is not guaranteed: a locked-down browser, a blocklisted GPU
//! driver, or a headless context can all refuse it. [`webgl2_available`]
//! asks before anything is mounted, and the preview renders its SVG pages
//! instead. The fallback is the ordinary path, not a degraded one — it is
//! what the rest of the site uses.

#[cfg(target_arch = "wasm32")]
pub use imp::*;

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use keyflow_ui::chart_renderer::ChartLayoutManager;
    use keyflow_ui::{ChartGraphics, PreviewMode};
    use kurbo::Affine;
    use wasm_bindgen::JsCast as _;

    /// The one canvas. A fixed id because there is only ever one live
    /// surface on the page — see the module note.
    pub const CANVAS_ID: &str = "kf-chart-surface";

    /// The renderer and the layout that feeds it.
    ///
    /// Neither is `Clone` or `Send`, and the surface must outlive the
    /// renders that draw into it, so it lives in an `Rc<RefCell<…>>` held
    /// by a `use_hook`. Single-threaded wasm, so the `RefCell` is honest.
    pub struct Surface {
        graphics: ChartGraphics,
        manager: ChartLayoutManager,
        /// What the manager was last laid out from, so a redraw caused by
        /// a pointer move does not re-parse the chart.
        laid_out: Option<(String, u32)>,
        /// The camera and hovered beat the canvas currently shows.
        ///
        /// A hover redraw is a redraw of everything — vello_hybrid builds
        /// the scene fresh each frame, so there is no "just repaint the
        /// highlight". Moving the pointer within one bar cannot change
        /// the picture, so it must not cost a frame: at four pages of
        /// vector geometry per draw, a pointer sweep across the stage
        /// otherwise queues hundreds of full renders and wedges the tab.
        shown: Option<(u64, Option<i64>)>,
    }

    pub type SurfaceCell = Rc<RefCell<Option<Surface>>>;

    #[must_use]
    pub fn surface_cell() -> SurfaceCell {
        Rc::new(RefCell::new(None))
    }

    /// Whether this browser will give us a WebGL2 context at all.
    ///
    /// Asked on a throwaway canvas that is never attached to the
    /// document, so a refusal costs nothing and leaves nothing behind.
    #[must_use]
    pub fn webgl2_available() -> bool {
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return false;
        };
        let Ok(el) = doc.create_element("canvas") else {
            return false;
        };
        let Ok(canvas) = el.dyn_into::<web_sys::HtmlCanvasElement>() else {
            return false;
        };
        matches!(canvas.get_context("webgl2"), Ok(Some(_)))
    }

    fn canvas() -> Option<web_sys::HtmlCanvasElement> {
        web_sys::window()?
            .document()?
            .get_element_by_id(CANVAS_ID)?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .ok()
    }

    /// Draw `source` into the surface, creating it on first use.
    ///
    /// `pan` and `zoom` are the camera the preview already tracks, handed
    /// to the renderer as a transform rather than applied as a CSS one —
    /// on a canvas there is no element to transform, and passing them in
    /// means the chart is re-rasterised at the new scale instead of being
    /// scaled up as pixels.
    ///
    /// Returns the readout for `hover`, if it landed on anything.
    pub fn draw(
        cell: &SurfaceCell,
        source: &str,
        pan: (f64, f64),
        zoom: f64,
        hover: Option<(f64, f64)>,
    ) -> Option<String> {
        let canvas = canvas()?;

        // The backing store is sized in device pixels; the element is
        // sized by CSS. Without the ratio the chart is soft on any
        // display that is not 1x.
        let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio());
        let css_w = canvas.client_width().max(1) as f64;
        let css_h = canvas.client_height().max(1) as f64;
        let (w, h) = ((css_w * dpr) as u32, (css_h * dpr) as u32);

        let mut borrow = cell.borrow_mut();
        let surface = match borrow.as_mut() {
            Some(s) => {
                if s.graphics.size() != (w, h) {
                    s.graphics.resize(w, h);
                    canvas.set_width(w);
                    canvas.set_height(h);
                }
                s
            }
            None => {
                canvas.set_width(w);
                canvas.set_height(h);
                let mut manager = ChartLayoutManager::new().ok()?;
                // Vector only. `vello_hybrid`'s WebGL2 painter asserts
                // inside `ImageSource::from_peniko_image_data` on the
                // level-of-detail rasters the manager produces for dense
                // multi-page views, and an assert in wasm aborts the
                // module — the page goes blank, not degraded. A four-page
                // chart trips the rule on its first frame.
                manager.set_raster_lod(false);
                let graphics = ChartGraphics::new_web(&canvas, w, h);
                *borrow = Some(Surface {
                    graphics,
                    manager,
                    laid_out: None,
                    shown: None,
                });
                borrow.as_mut()?
            }
        };

        // Lay out only when the source or the width it was laid out for
        // has changed. A pointer move must not re-parse the chart.
        let key = (source.to_string(), w);
        if surface.laid_out.as_ref() != Some(&key) {
            if surface
                .manager
                .parse_and_layout_with_preview_mode(source, f64::from(w), PreviewMode::Page, 1.0)
                .is_err()
            {
                return None;
            }
            surface.laid_out = Some(key);
        }

        let camera = Affine::translate((pan.0 * dpr, pan.1 * dpr)) * Affine::scale(zoom * dpr);

        let hit = hover.and_then(|(x, y)| surface.manager.hit_test_at_point(x, y));
        let tick = hit.as_ref().map(|h| h.absolute_tick);
        let readout = tick.map(|t| surface.manager.musical_position_for_tick(t));

        // Nothing about the picture has changed — same camera, same
        // hovered beat — so there is nothing to draw.
        let camera_key = camera_key(camera, w, h);
        if surface.shown == Some((camera_key, tick)) {
            return readout;
        }
        surface.shown = Some((camera_key, tick));

        let Surface {
            graphics, manager, ..
        } = surface;
        graphics.render_chart(|painter| {
            manager.render_to_scene(
                painter,
                f64::from(w),
                f64::from(h),
                Affine::IDENTITY,
                camera,
                None,
                hover,
            );
        });

        readout
    }

    /// A hashable stand-in for "what the camera is currently showing".
    ///
    /// `Affine` is six `f64`s and not `Hash`; the bits are compared
    /// directly because the question is only ever "is this the same
    /// camera as last frame", and the same camera produces identical
    /// bits.
    fn camera_key(camera: Affine, w: u32, h: u32) -> u64 {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for c in camera.as_coeffs() {
            c.to_bits().hash(&mut hasher);
        }
        w.hash(&mut hasher);
        h.hash(&mut hasher);
        hasher.finish()
    }

    /// Turn a pointer position inside the canvas into a scene point.
    ///
    /// The inverse of the camera: the renderer draws the scene through
    /// `translate(pan) * scale(zoom)`, so a point on screen is that
    /// transform undone. Done in CSS pixels — the device-pixel ratio
    /// cancels, because it scales the pointer and the camera alike.
    #[must_use]
    pub fn scene_point(client: (f64, f64), pan: (f64, f64), zoom: f64) -> Option<(f64, f64)> {
        let rect = canvas()?.get_bounding_client_rect();
        let x = (client.0 - rect.left() - pan.0) / zoom;
        let y = (client.1 - rect.top() - pan.1) / zoom;
        Some((x, y))
    }

    /// Drop the surface, releasing its WebGL context.
    ///
    /// Contexts are the scarce resource here, and a browser does not
    /// reclaim one promptly just because the canvas left the document.
    pub fn release(cell: &SurfaceCell) {
        *cell.borrow_mut() = None;
    }
}

// The site is checked for the host as well as for wasm, and none of the
// above exists there: `ChartGraphics`'s web backend is behind
// `target_arch = "wasm32"`. The preview asks `webgl2_available` before it
// mounts anything, so on the host it simply always renders SVG.
#[cfg(not(target_arch = "wasm32"))]
pub use stub::*;

#[cfg(not(target_arch = "wasm32"))]
mod stub {
    use std::cell::RefCell;
    use std::rc::Rc;

    pub const CANVAS_ID: &str = "kf-chart-surface";
    pub struct Surface;
    pub type SurfaceCell = Rc<RefCell<Option<Surface>>>;

    #[must_use]
    pub fn surface_cell() -> SurfaceCell {
        Rc::new(RefCell::new(None))
    }

    #[must_use]
    pub const fn webgl2_available() -> bool {
        false
    }

    pub fn draw(
        _cell: &SurfaceCell,
        _source: &str,
        _pan: (f64, f64),
        _zoom: f64,
        _hover: Option<(f64, f64)>,
    ) -> Option<String> {
        None
    }

    #[must_use]
    pub const fn scene_point(
        _client: (f64, f64),
        _pan: (f64, f64),
        _zoom: f64,
    ) -> Option<(f64, f64)> {
        None
    }

    pub fn release(_cell: &SurfaceCell) {}
}
