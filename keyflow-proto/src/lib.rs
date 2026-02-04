//! Keyflow Protocol - Shared types and service definitions for Keyflow cells
//!
//! This crate defines the data types and service interfaces for music chart operations.
//! All types are Facet-derived for RPC compatibility.

#![deny(unsafe_code)]

// Domain modules
pub mod api;
pub mod chart;
pub mod chord;
pub mod core;
pub mod key;
pub mod metadata;
pub mod parsing;
pub mod primitives;
pub mod sections;
pub mod services;
pub mod time;

// AST module (for syntax tree types)
pub mod ast;

// Syntax highlighting module - parser-integrated highlighting
#[cfg(feature = "highlighting")]
pub mod highlighting;

// Re-export common types for convenience
pub use api::prelude as api_prelude;
pub use chart::{
    Chart, ChartIndex, ChartPosition, ChartSection, ChordInstance, DynamicMarking, ElementId,
    KeyChange, Measure, NavigationType, SemanticRole, SourceLink, TempoChange, TextCue,
    TimeSignatureChange,
};

pub use chord::{
    Alteration, Chord, ChordDegree, ChordFamily, ChordParseError, ChordParseErrors, ChordQuality,
    ChordRhythm, DetailLevel, ExtensionQuality, Extensions, LilySyntax, PushPullAmount,
    RootParseResult, SuspendedType, UpperStructure, parse_root,
};

pub use key::{Key, ScaleMode, ScaleType};

pub use metadata::SongMetadata;

pub use parsing::{ParseError, TextSpan, Token, TokenType};

pub use primitives::{
    Interval, MusicalNote, MusicalNoteToken, Note, RomanCase, RomanNumeralToken, RootFormat,
    RootNotation, ScaleDegreeToken,
};

pub use sections::{Section, SectionNumberer, SectionType};

pub use time::{
    AbsolutePosition, Duration, MusicalDuration, MusicalPosition, PPQDuration,
    PPQPosition, Position, Tempo, TimeDuration, TimePosition, TimeSignature,
};

pub use ast::{
    AccidentalAst, AlterationAst, AstNode, BassToneAst, ChordAst, ChordModifierAst, DurationAst,
    ExtensionAst, ExtensionQualityAst, QualityAst, RhythmAst, RhythmKind, RootAst, SlashCountAst,
    Spanned,
};

// Service types
pub use services::{
    ChartParseService, ChartParseServiceClient, ChartParseServiceDispatcher, ChartService,
    ChartServiceClient, ChartServiceDispatcher, ParseRequest, ParseResponse, ParserService,
    ParserServiceClient, ParserServiceDispatcher,
};
