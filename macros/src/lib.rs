//! Procedural macro for 256-bit integer literals.
//!
//! See [`ethnum::int`](https://docs.rs/ethuint/latest/ethuint/macro.int.html)
//! and [`ethnum::uint`](https://docs.rs/ethnum/latest/ethnum/macro.uint.html)
//! documentation for more information.

extern crate proc_macro;

mod parse;

use self::parse::{Int, LiteralError};
use parse::Uint;
use proc_macro::{Delimiter, Literal, Span, TokenStream, TokenTree};

#[proc_macro]
pub fn int(input: TokenStream) -> TokenStream {
    match IntLiteral::generate(input) {
        Ok(value) => value.into_tokens(),
        Err(err) => err.into_tokens(),
    }
}

#[proc_macro]
pub fn uint(input: TokenStream) -> TokenStream {
    match UintLiteral::generate(input) {
        Ok(value) => value.into_tokens(),
        Err(err) => err.into_tokens(),
    }
}

struct IntLiteral(Int);

impl IntLiteral {
    fn generate(input: TokenStream) -> Result<Self, CompileError> {
        let input = Input::parse(input)?;
        Ok(Self(Int::from_literal(&input.value)?))
    }

    fn into_tokens(self) -> TokenStream {
        let (hi, lo) = self.0.into_words();
        format!("::ethnum::I256::from_words({hi}, {lo})")
            .parse()
            .unwrap()
    }
}

struct UintLiteral(Uint);

impl UintLiteral {
    fn generate(input: TokenStream) -> Result<Self, CompileError> {
        let input = Input::parse(input)?;
        Ok(Self(Uint::from_literal(&input.value)?))
    }

    fn into_tokens(self) -> TokenStream {
        let (hi, lo) = self.0.into_words();
        format!("::ethnum::U256::from_words({hi}, {lo})")
            .parse()
            .unwrap()
    }
}

struct Input {
    value: String,
    span: Span,
}

impl Input {
    fn parse(input: TokenStream) -> Result<Self, CompileError> {
        let mut result = Input {
            value: String::new(),
            span: Span::call_site(),
        };
        ParserState::start().input(input, &mut result)?.end()?;

        Ok(result)
    }
}

enum ParserState {
    String,
    Eof,
}

impl ParserState {
    fn start() -> Self {
        Self::String
    }

    fn input(self, input: TokenStream, result: &mut Input) -> Result<Self, CompileError> {
        input
            .into_iter()
            .try_fold(self, |state, token| state.next(token, result))
    }

    fn next(self, token: TokenTree, result: &mut Input) -> Result<Self, CompileError> {
        match (&self, &token) {
            // Procedural macros invoked from withing `macro_rules!` expansions
            // may be grouped with a `Ø` delimiter (which allows operator
            // precidence to be preserved).
            //
            // See <https://doc.rust-lang.org/stable/proc_macro/enum.Delimiter.html#variant.None>
            (_, TokenTree::Group(g)) if g.delimiter() == Delimiter::None => {
                self.input(g.stream(), result)
            }

            (Self::String, TokenTree::Literal(l)) => match parse_string(l) {
                Some(value) => {
                    result.value = value;
                    result.span = token.span();
                    Ok(Self::Eof)
                }
                None => Err(self.unexpected(Some(token))),
            },

            _ => Err(self.unexpected(Some(token))),
        }
    }

    fn end(self) -> Result<(), CompileError> {
        match self {
            ParserState::Eof => Ok(()),
            _ => Err(self.unexpected(None)),
        }
    }

    fn unexpected(self, token: Option<TokenTree>) -> CompileError {
        let expected = match self {
            ParserState::String => "string literal",
            ParserState::Eof => "<eof>",
        };
        let (value, span) = match token {
            Some(TokenTree::Group(g)) => {
                let delim = match g.delimiter() {
                    Delimiter::Parenthesis => "(",
                    Delimiter::Brace => "{",
                    Delimiter::Bracket => "[",
                    Delimiter::None => "Ø",
                };
                (delim.to_string(), Some(g.span_open()))
            }
            Some(t) => (t.to_string(), Some(t.span())),
            None => ("<eof>".to_owned(), None),
        };

        CompileError {
            message: format!("expected {expected} but found `{value}`"),
            span,
        }
    }
}

struct CompileError {
    message: String,
    span: Option<Span>,
}

impl CompileError {
    fn into_tokens(self) -> TokenStream {
        let error = format!("compile_error!({:?})", self.message)
            .parse::<TokenStream>()
            .unwrap();

        match self.span {
            Some(span) => error
                .into_iter()
                .map(|mut token| {
                    token.set_span(span);
                    token
                })
                .collect(),
            None => error,
        }
    }
}

impl From<LiteralError> for CompileError {
    fn from(err: LiteralError) -> Self {
        Self {
            message: err.to_string(),
            span: None,
        }
    }
}

fn parse_string(literal: &Literal) -> Option<String> {
    Some(
        literal
            .to_string()
            .strip_prefix('"')?
            .strip_suffix('"')?
            .to_owned(),
    )
}