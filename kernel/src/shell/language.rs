/// A parsed bootstrap-shell line. After trimming outer and separator whitespace,
/// arguments remain literal: milestone zero does not interpret quoting,
/// expansion, substitution, redirection, or pipelines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct parsed_line<'a> {
    pub command: &'a str,
    pub arguments: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum builtin_kind {
    help,
    version,
    about,
    clear,
    echo,
    #[cfg(any(feature = "desktop_dev", feature = "workspace_dev"))]
    desktop,
    unknown,
}

impl builtin_kind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::help => "help",
            Self::version => "version",
            Self::about => "about",
            Self::clear => "clear",
            Self::echo => "echo",
            #[cfg(any(feature = "desktop_dev", feature = "workspace_dev"))]
            Self::desktop => "desktop",
            Self::unknown => "unknown",
        }
    }
}

#[must_use]
pub fn parse(line: &str) -> Option<parsed_line<'_>> {
    let line = line.trim_matches(char::is_whitespace);
    if line.is_empty() {
        return None;
    }
    let command_end = line.find(char::is_whitespace).unwrap_or(line.len());
    Some(parsed_line {
        command: &line[..command_end],
        arguments: line[command_end..].trim_start_matches(char::is_whitespace),
    })
}

#[must_use]
pub fn classify(command: &str) -> builtin_kind {
    if command.eq_ignore_ascii_case("help") {
        builtin_kind::help
    } else if command.eq_ignore_ascii_case("version") {
        builtin_kind::version
    } else if command.eq_ignore_ascii_case("about") {
        builtin_kind::about
    } else if command.eq_ignore_ascii_case("clear") {
        builtin_kind::clear
    } else if command.eq_ignore_ascii_case("echo") {
        builtin_kind::echo
    } else {
        builtin_kind::unknown
    }
}

#[must_use]
pub fn classify_for_session(command: &str, return_to_desktop: bool) -> builtin_kind {
    #[cfg(any(feature = "desktop_dev", feature = "workspace_dev"))]
    if return_to_desktop && command.eq_ignore_ascii_case("desktop") {
        return builtin_kind::desktop;
    }
    let _ = return_to_desktop;
    classify(command)
}

#[cfg(test)]
mod tests {
    use super::{builtin_kind, classify, classify_for_session, parse, parsed_line};

    #[test]
    fn parse_separates_literal_arguments() {
        assert_eq!(
            parse("  echo   one two  "),
            Some(parsed_line {
                command: "echo",
                arguments: "one two",
            })
        );
    }

    #[test]
    fn parse_rejects_blank_lines() {
        assert_eq!(parse(" \t "), None);
    }

    #[test]
    fn classification_is_ascii_case_insensitive() {
        assert_eq!(classify("HeLp"), builtin_kind::help);
        assert_eq!(classify("help!"), builtin_kind::unknown);
    }

    #[test]
    fn desktop_is_not_an_ordinary_shell_command() {
        assert_eq!(classify("desktop"), builtin_kind::unknown);
        assert_eq!(
            classify_for_session("desktop", false),
            builtin_kind::unknown
        );
        assert_eq!(
            classify_for_session("desktop!", true),
            builtin_kind::unknown
        );
    }

    #[cfg(any(feature = "desktop_dev", feature = "workspace_dev"))]
    #[test]
    fn desktop_requires_the_development_session() {
        assert_eq!(classify_for_session("DeSkToP", true), builtin_kind::desktop);
        assert_eq!(classify_for_session("echo", true), builtin_kind::echo);
    }
}
