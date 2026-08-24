//! Pure string expander module for variable substitution.
//!
//! Performs single-pass variable substitution with support for:
//! - `$NAME` — simple variable reference
//! - `${NAME}` — braced variable reference
//! - `$$` — escaped dollar sign (emits single `$`)

use thiserror::Error;

/// Error types for string expansion.
#[derive(Debug, Error, PartialEq)]
pub enum ExpandError {
    /// A variable was referenced but not found in the lookup function.
    #[error("undefined variable: {0}")]
    MissingVariable(String),

    /// An opening `${` was not closed with `}`.
    #[error("unterminated brace in variable reference")]
    UnterminatedBrace,
}

/// Expands variables in a string using a single pass.
///
/// Performs variable substitution for `$NAME`, `${NAME}`, and `$$` patterns.
/// Does not re-scan output, ensuring single-pass performance.
///
/// # Arguments
///
/// * `input` - The string to expand
/// * `lookup` - A function that returns the value for a variable name, or None if not found
///
/// # Returns
///
/// - `Ok(String)` with the expanded text
/// - `Err(ExpandError::MissingVariable(name))` if a variable is not found
/// - `Err(ExpandError::UnterminatedBrace)` if `${` is not closed with `}`
pub fn expand(input: &str, lookup: impl Fn(&str) -> Option<String>) -> Result<String, ExpandError> {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            // Check for $$
            if chars.peek() == Some(&'$') {
                chars.next(); // consume the second $
                result.push('$'); // emit a single $
            } else if chars.peek() == Some(&'{') {
                // Handle ${NAME}
                chars.next(); // consume the {
                let mut var_name = String::new();

                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => var_name.push(c),
                        None => return Err(ExpandError::UnterminatedBrace),
                    }
                }

                // Look up the variable
                let value = lookup(&var_name)
                    .ok_or_else(|| ExpandError::MissingVariable(var_name.clone()))?;
                result.push_str(&value);
            } else {
                // Handle $NAME
                let mut var_name = String::new();

                // Variable names consist of alphanumeric characters and underscores
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_alphanumeric() || next_ch == '_' {
                        var_name.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if var_name.is_empty() {
                    // No variable name after $, emit the $ literally
                    result.push('$');
                } else {
                    // Look up the variable
                    let value = lookup(&var_name)
                        .ok_or_else(|| ExpandError::MissingVariable(var_name.clone()))?;
                    result.push_str(&value);
                }
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_variable_expansion() {
        let result = expand(
            "$FOO",
            |n| {
                if n == "FOO" {
                    Some("bar".into())
                } else {
                    None
                }
            },
        );
        assert_eq!(result, Ok("bar".to_string()));
    }

    #[test]
    fn test_braced_variable_with_surrounding_text() {
        let result = expand("hello_${FOO}_world", |n| {
            if n == "FOO" {
                Some("bar".into())
            } else {
                None
            }
        });
        assert_eq!(result, Ok("hello_bar_world".to_string()));
    }

    #[test]
    fn test_back_to_back_braced_variables() {
        let result = expand("${A}${B}", |n| {
            if n == "A" {
                Some("foo".into())
            } else if n == "B" {
                Some("baz".into())
            } else {
                None
            }
        });
        assert_eq!(result, Ok("foobaz".to_string()));
    }

    #[test]
    fn test_escaped_dollar_sign() {
        let result = expand("$$", |_n| None);
        assert_eq!(result, Ok("$".to_string()));
    }

    #[test]
    fn test_escaped_dollar_followed_by_variable() {
        let result = expand(
            "$$$FOO",
            |n| {
                if n == "FOO" {
                    Some("bar".into())
                } else {
                    None
                }
            },
        );
        assert_eq!(result, Ok("$bar".to_string()));
    }

    #[test]
    fn test_unterminated_brace() {
        let result = expand("${FOO", |_n| None);
        assert_eq!(result, Err(ExpandError::UnterminatedBrace));
    }

    #[test]
    fn test_missing_variable() {
        let result = expand("$MISSING", |_n| None);
        assert_eq!(
            result,
            Err(ExpandError::MissingVariable("MISSING".to_string()))
        );
    }
}
