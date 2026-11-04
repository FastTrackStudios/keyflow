//! Native source editor and live chart in a workspace pane.

use architect_workspace::paint::PaintPane;
use architect_workspace::HostData;
use dioxus::prelude::*;
use editor_keyflow::view::KeyflowEditor;

use crate::paint_view::{ChartMount, ChartSnapshot};

/// Mount within a workspace pane after registering the paint view kind.
/// `data` holds the last successfully parsed chart, theme and playhead; the
/// host may update it and wake the chart for playback or theme changes.
/// Edits that do not parse keep the last valid chart visible.
#[component]
pub fn NativeChartEditor(
    initial: String,
    data: HostData<ChartSnapshot>,
    chart: ChartMount,
    #[props(default)] highlight_theme: Option<editor_keyflow::lang::HighlightTheme>,
    /// CSS custom properties for the editor chrome and caret.
    #[props(default)]
    editor_style: Option<String>,
    on_change: EventHandler<String>,
) -> Element {
    let wake = chart.wake.clone();
    let source_style = format!(
        "flex: 1; min-width: 0; min-height: 0; {}",
        editor_style.unwrap_or_default()
    );
    rsx! {
        div { style: "display: flex; width: 100%; height: 100%; min-height: 0;",
            div { style: "{source_style}",
                KeyflowEditor {
                    initial,
                    highlight_theme,
                    on_change: move |source: String| {
                        if let Ok(parsed) = keyflow::parse(&source) {
                            data.update(|snapshot| {
                                snapshot.chart = parsed;
                                snapshot.revision = snapshot.revision.wrapping_add(1);
                            });
                            wake.wake();
                        }
                        on_change.call(source);
                    },
                }
            }
            div { style: "flex: 1; min-width: 0; min-height: 0;",
                PaintPane { mount: chart.mount.clone() }
            }
        }
    }
}
