//! Chart macro for ergonomic chart definition
//!
//! Provides a convenient `chart!` macro for defining charts inline

/// Define a chart using a natural text-based syntax
///
/// # Example
/// ```
/// use keyflow_proto::chart;
///
/// let my_chart = chart! {"
///     Reckless Love - Cory Asbury
///     68bpm 6/8 #G
///     
///     in
///     6 5 4 1
///     
///     vs
///     6 5 4 4 x4
///     
///     ch
///     6 5 4 1 x4
/// "};
/// ```
#[macro_export]
macro_rules! chart {
    // Accept either a string literal directly
    ($chart_text:expr) => {{
        $crate::chart::Chart::parse($chart_text)
    }};

    // Or accept raw content and stringify it (for multi-line without quotes)
    ($($tt:tt)*) => {{
        $crate::chart::Chart::parse(stringify!($($tt)*))
    }};
}
