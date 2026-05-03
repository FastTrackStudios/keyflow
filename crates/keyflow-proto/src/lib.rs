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
pub mod guide;
pub mod key;
pub mod metadata;
pub mod parsing;
pub mod primitives;
pub mod sections;
pub mod services;
pub mod time;

// Multi-block document support
#[cfg(feature = "serde")]
pub mod document;

// AST module (for syntax tree types)
pub mod ast;

// Syntax highlighting module - parser-integrated highlighting
#[cfg(feature = "highlighting")]
pub mod highlighting;

// Re-export common types for convenience
pub use api::prelude as api_prelude;
pub use chart::{
    Chart, ChartIndex, ChartPosition, ChartSection, ChordAttachment, ChordAttachmentType,
    ChordInstance, ChordSyllableAligner, ChordSyllableMapping, DynamicMarking, ElementId,
    KeyChange, LyricChordParser, LyricLine, LyricSourceFormat, LyricSyllable, LyricSyncLevel,
    Measure, NavigationType, SectionAlignment, SemanticRole, SourceLink, SyllableParser,
    TempoChange, TextCue, TimeSignatureChange,
};

#[cfg(feature = "serde")]
pub use document::{KfBlock, KfBlockKind, KfDocument};

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

pub use guide::{
    ClickConfig, ClickEvent, ClickType, CountEvent, CountInConfig, CountInState, GuideConfig,
    GuideEvent, SectionCueEvent,
};

pub use sections::{Section, SectionNumberer, SectionType};

pub use time::{
    AbsolutePosition, Duration, MusicalDuration, MusicalPosition, PPQDuration, PPQPosition,
    Position, Tempo, TimeDuration, TimePosition, TimeSignature,
};

pub use ast::{
    AccidentalAst, AlterationAst, AstNode, BassToneAst, ChordAst, ChordModifierAst, DurationAst,
    ExtensionAst, ExtensionQualityAst, QualityAst, RhythmAst, RhythmKind, RootAst, SlashCountAst,
    Spanned,
};

// Service types
pub use services::{
    ChartParseService, ChartParseServiceClient, ChartParseServiceDispatcher, ChartService,
    ChartServiceClient, ChartServiceDispatcher, GuideRequest, GuideService, GuideServiceClient,
    GuideServiceDispatcher, ParseRequest, ParseResponse, ParserService, ParserServiceClient,
    ParserServiceDispatcher,
};
