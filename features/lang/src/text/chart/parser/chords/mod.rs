//! Chord parsing for charts
//!
//! Handles parsing of chord lines, individual chord tokens, and related
//! functionality including duration calculation, slash chords, and push/pull notation.
//!
//! The chart body parser, by responsibility:
//!
//! - `section` — a section's content lines, and the `parse_chord_line*`
//!   entry points.
//! - `line` — one chord line into measures (the token loop).
//! - `repeats`, `parallel` — phrase repeats and barred lines; parallel
//!   `<< >>` containers.
//! - `chord_token`, `notation`, `modifiers`, `figures` — a single
//!   chord token: its notation system, push/pull marks and figures.
//! - `measure`, `auto_duration`, `groups`, `tokens` — measure
//!   bookkeeping, durations, `()` groups and token helpers.
//! - `annotations` — aliases, staff text, dynamics and hairpins.

mod annotations;
mod auto_duration;
mod chord_token;
mod figures;
mod groups;
mod line;
mod measure;
mod modifiers;
mod notation;
mod parallel;
mod repeats;
mod section;
mod tokens;

#[cfg(test)]
mod tests;
