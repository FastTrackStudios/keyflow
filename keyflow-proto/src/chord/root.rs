//! Root notation parsing
//!
//! Handles parsing of chord roots from tokens (note names, scale degrees, roman numerals)

use crate::parsing::{ParseError, Token, TokenType};
use crate::primitives::{MusicalNoteToken, RomanNumeralToken, RootNotation, ScaleDegreeToken};

/// Result of parsing a root notation with token consumption info
#[derive(Debug, Clone, PartialEq)]
pub struct RootParseResult {
    /// The parsed root notation
    pub root: RootNotation,
    /// Number of tokens consumed from input
    pub tokens_consumed: usize,
}

impl RootParseResult {
    pub fn new(root: RootNotation, tokens_consumed: usize) -> Self {
        Self {
            root,
            tokens_consumed,
        }
    }
}

/// Parse root notation from tokens
///
/// Tries to parse as note name, scale degree, or roman numeral based on token patterns.
/// Returns the parsed root and number of tokens consumed.
pub fn parse_root(tokens: &[Token]) -> Result<RootParseResult, ParseError> {
    if tokens.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    // Skip whitespace
    let tokens = skip_whitespace(tokens);
    if tokens.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    // Try to determine which parser to use based on token pattern
    let parse_order = detect_parser_order(tokens);

    for parser_type in parse_order {
        match parser_type {
            ParserType::ScaleDegree => {
                if let Some(result) = ScaleDegreeToken::parse(tokens) {
                    let root = RootNotation::from_scale_degree(result.degree, result.accidental);
                    return Ok(RootParseResult::new(root, result.tokens_consumed));
                }
            }
            ParserType::Roman => {
                if let Some(result) = RomanNumeralToken::parse(tokens) {
                    let consumed = result.tokens_consumed;
                    let root = RootNotation::from_roman_token(result);
                    return Ok(RootParseResult::new(root, consumed));
                }
            }
            ParserType::NoteName => {
                if let Some(result) = MusicalNoteToken::parse(tokens) {
                    let consumed = result.tokens_consumed;
                    let root = RootNotation::from_note_token(result);
                    return Ok(RootParseResult::new(root, consumed));
                }
            }
        }
    }

    Err(ParseError::NoValidParser {
        context: format!(
            "Unable to parse root from tokens starting with: {:?}",
            tokens.first()
        ),
    })
}

/// Detect the order in which to try parsers based on token patterns
fn detect_parser_order(tokens: &[Token]) -> Vec<ParserType> {
    if tokens.is_empty() {
        return vec![
            ParserType::NoteName,
            ParserType::Roman,
            ParserType::ScaleDegree,
        ];
    }

    let first_token = &tokens[0].token_type;

    match first_token {
        // Accidental followed by...
        TokenType::Sharp | TokenType::Flat => {
            if let Some(second) = tokens.get(1) {
                match &second.token_type {
                    // Accidental + Number -> Scale Degree first
                    TokenType::Number(s) if is_scale_degree_number(s) => {
                        vec![
                            ParserType::ScaleDegree,
                            ParserType::NoteName,
                            ParserType::Roman,
                        ]
                    }
                    // Accidental + Letter -> Note Name first (could be Roman too)
                    TokenType::Letter(_) => {
                        vec![
                            ParserType::NoteName,
                            ParserType::Roman,
                            ParserType::ScaleDegree,
                        ]
                    }
                    _ => vec![
                        ParserType::NoteName,
                        ParserType::Roman,
                        ParserType::ScaleDegree,
                    ],
                }
            } else {
                vec![
                    ParserType::NoteName,
                    ParserType::Roman,
                    ParserType::ScaleDegree,
                ]
            }
        }

        // Number -> Scale Degree first
        TokenType::Number(s) if is_scale_degree_number(s) => {
            vec![
                ParserType::ScaleDegree,
                ParserType::NoteName,
                ParserType::Roman,
            ]
        }

        // Letter -> Could be Note Name or Roman Numeral
        TokenType::Letter(c) => {
            if is_roman_numeral_letter(*c) {
                // Try Roman first, then Note Name
                vec![
                    ParserType::Roman,
                    ParserType::NoteName,
                    ParserType::ScaleDegree,
                ]
            } else {
                // Try Note Name first
                vec![
                    ParserType::NoteName,
                    ParserType::Roman,
                    ParserType::ScaleDegree,
                ]
            }
        }

        // Default order
        _ => vec![
            ParserType::NoteName,
            ParserType::Roman,
            ParserType::ScaleDegree,
        ],
    }
}

/// Check if a number string is a valid scale degree (1-7)
fn is_scale_degree_number(s: &str) -> bool {
    if let Ok(num) = s.parse::<u8>() {
        (1..=7).contains(&num)
    } else {
        false
    }
}

/// Check if a letter could be part of a Roman numeral
fn is_roman_numeral_letter(c: char) -> bool {
    matches!(c.to_ascii_uppercase(), 'I' | 'V' | 'X')
}

/// Skip leading whitespace tokens
fn skip_whitespace(tokens: &[Token]) -> &[Token] {
    let mut i = 0;
    while i < tokens.len() && tokens[i].token_type == TokenType::Space {
        i += 1;
    }
    &tokens[i..]
}

/// Parser type enum for determining which mini-parser to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserType {
    ScaleDegree,
    Roman,
    NoteName,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::Lexer;

    #[test]
    fn test_parse_note_name() {
        let mut lexer = Lexer::new("C".to_string());
        let tokens = lexer.tokenize();
        let result = parse_root(&tokens).unwrap();

        assert_eq!(result.tokens_consumed, 1);
    }

    #[test]
    fn test_parse_sharp_note() {
        let mut lexer = Lexer::new("F#".to_string());
        let tokens = lexer.tokenize();
        let result = parse_root(&tokens).unwrap();

        assert_eq!(result.tokens_consumed, 2);
    }

    #[test]
    fn test_parse_scale_degree() {
        let mut lexer = Lexer::new("4".to_string());
        let tokens = lexer.tokenize();
        let result = parse_root(&tokens).unwrap();

        assert_eq!(result.tokens_consumed, 1);
    }

    #[test]
    fn test_parse_sharp_scale_degree() {
        let mut lexer = Lexer::new("#4".to_string());
        let tokens = lexer.tokenize();
        let result = parse_root(&tokens).unwrap();

        assert_eq!(result.tokens_consumed, 2);
    }

    #[test]
    fn test_parse_roman_numeral() {
        let mut lexer = Lexer::new("IV".to_string());
        let tokens = lexer.tokenize();
        let result = parse_root(&tokens).unwrap();

        assert_eq!(result.tokens_consumed, 2);
    }

    #[test]
    fn test_parse_lowercase_roman() {
        let mut lexer = Lexer::new("vi".to_string());
        let tokens = lexer.tokenize();
        let result = parse_root(&tokens).unwrap();

        assert_eq!(result.tokens_consumed, 2);
    }

    #[test]
    fn test_skip_whitespace() {
        let mut lexer = Lexer::new("  C".to_string());
        let tokens = lexer.tokenize();
        let result = parse_root(&tokens).unwrap();

        assert_eq!(result.tokens_consumed, 1);
    }
}
