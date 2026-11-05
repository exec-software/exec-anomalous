use crate::program_startup::{granted_handle, service};

/// Writing text out of a program-bearing task.
///
/// The first service ADR 0092 names, and the one `hello.nil` needs. It is a
/// protocol rather than a syscall: this crate forbids unsafe code and has no
/// kernel, so the transport is supplied and this decides only what a legal
/// write is.
///
/// WHAT A CONSOLE WRITE MAY CONTAIN, AND WHY IT MATTERS
///
/// A console is not a byte sink. Terminals interpret what arrives: escape
/// sequences move the cursor, clear the screen, set the window title, and on
/// several widely used terminals can query or alter the clipboard and report
/// the result back on the input stream. A program that merely prints is
/// therefore a program that can drive the operator's terminal, and on a system
/// whose whole claim is that an agent's authority is bounded, "it only wrote
/// to stdout" is not a bound.
///
/// So this refuses control bytes rather than passing them through. Newline and
/// tab are admitted because text has lines and columns; everything else in C0,
/// the DEL byte and the C1 range are refused. The write fails whole: a
/// truncating write would leave the operator reading half a sentence and
/// guessing whether the rest was lost or never existed.
///
/// Text is UTF-8, validated here. A console that is handed invalid sequences
/// renders them however it likes, which is another way for output to say
/// something the program did not.
pub const maximum_write_bytes: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum console_error {
    /// The handle was granted for a different service.
    family,
    /// Nothing to write. Distinguished from a refusal so a caller can tell
    /// "you wrote nothing" from "you wrote something inadmissible".
    empty,
    /// Longer than one write admits.
    length,
    /// A control or escape byte the terminal would act on.
    control,
    /// Not valid UTF-8.
    encoding,
    /// The transport refused or failed.
    transport,
}

/// Somewhere for bytes to go. Implemented by the task's kernel stub in a
/// crate that is allowed unsafe code, and by a recorder in tests.
pub trait console_transport {
    fn write(&mut self, channel: u64, bytes: &[u8], sequence: u64) -> Result<usize, console_error>;
}

pub struct console_writer<transport: console_transport> {
    handle: granted_handle,
    pub(crate) transport: transport,
    sequence: u64,
}

impl<transport: console_transport> console_writer<transport> {
    /// A writer exists only where a launch granted a console.
    ///
    /// The handle cannot be constructed outside its own module, so reaching
    /// this function at all is the proof. The family is checked anyway: a
    /// handle granted for file reading is a real handle, and passing it here
    /// should fail rather than write a program's output somewhere it was never
    /// meant to go.
    pub fn new(handle: granted_handle, transport: transport) -> Result<Self, console_error> {
        if handle.family() != service::console_write {
            return Err(console_error::family);
        }
        Ok(Self {
            handle,
            transport,
            sequence: 1,
        })
    }

    /// Writes already issued, so a caller can report progress without the
    /// transport having to.
    pub const fn written(&self) -> u64 {
        self.sequence - 1
    }

    /// Write one whole piece of text.
    ///
    /// Refused as a whole or written as a whole. A partial write from the
    /// transport is a transport failure rather than a shorter success: the
    /// caller has no way to resume that is not a second, differently sequenced
    /// write, and silently losing a tail is the failure this would exist to
    /// prevent.
    pub fn write(&mut self, text: &[u8]) -> Result<(), console_error> {
        if text.is_empty() {
            return Err(console_error::empty);
        }
        if text.len() > maximum_write_bytes {
            return Err(console_error::length);
        }
        // Encoding is settled before control characters are looked for, and
        // the order is not cosmetic. A UTF-8 continuation byte is 0x80..=0xbf,
        // which overlaps the C1 range exactly, so a raw-byte scan rejects every
        // multi-byte character -- an em dash reads as an escape. Decode first,
        // then judge characters.
        let text = match core::str::from_utf8(text) {
            Ok(text) => text,
            Err(_) => return Err(console_error::encoding),
        };
        if text.chars().any(is_control) {
            return Err(console_error::control);
        }
        let text = text.as_bytes();
        match self.transport.write(self.handle.raw(), text, self.sequence) {
            Ok(written) if written == text.len() => {
                self.sequence += 1;
                Ok(())
            }
            Ok(_) => Err(console_error::transport),
            Err(error) => Err(error),
        }
    }
}

/// Everything a terminal acts on rather than shows, except newline and tab.
///
/// `char::is_control` is exactly C0, DEL and C1, which is the set wanted here.
/// C1 matters even though a program is unlikely to emit it deliberately: U+009B
/// is CSI, and a filter watching only for U+001B would pass it.
///
/// Taking characters rather than bytes is what makes this correct for text
/// that is not ASCII; see the note at the call site.
fn is_control(character: char) -> bool {
    character.is_control() && character != '\n' && character != '\t'
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::program_descriptor;
    use crate::program_startup::{minimum_arena_bytes, service};
    use std::vec::Vec;

    #[derive(Default)]
    struct recorder {
        writes: Vec<(u64, Vec<u8>, u64)>,
        short_by: usize,
        fail: bool,
    }

    impl console_transport for recorder {
        fn write(
            &mut self,
            channel: u64,
            bytes: &[u8],
            sequence: u64,
        ) -> Result<usize, console_error> {
            if self.fail {
                return Err(console_error::transport);
            }
            self.writes.push((channel, bytes.to_vec(), sequence));
            Ok(bytes.len() - self.short_by)
        }
    }

    fn console() -> granted_handle {
        program_descriptor::try_new(
            0x1000_0000,
            minimum_arena_bytes,
            7,
            334,
            9,
            service::console_write,
            b"",
        )
        .expect("a launch that grants a console")
        .console()
        .expect("console_write was granted")
    }

    #[test]
    fn text_reaches_the_transport_whole_and_in_order() {
        let mut writer = console_writer::new(console(), recorder::default()).expect("granted");
        writer
            .write(b"hello from an edition\n")
            .expect("ordinary text");
        writer.write(b"and a second line\n").expect("ordinary text");
        assert_eq!(writer.written(), 2);
        let recorder = writer.transport;
        assert_eq!(recorder.writes.len(), 2);
        assert_eq!(
            recorder.writes[0].0, 9,
            "the granted channel, not any other"
        );
        assert_eq!(recorder.writes[0].2, 1);
        assert_eq!(
            recorder.writes[1].2, 2,
            "sequence advances once per accepted write"
        );
    }

    #[test]
    fn an_escape_sequence_is_refused_rather_than_passed_through() {
        // The case this service exists for. A terminal acts on what arrives:
        // this would clear the operator's screen, and 0x1b is how every one of
        // those sequences begins.
        let mut writer = console_writer::new(console(), recorder::default()).expect("granted");
        assert_eq!(writer.write(b"\x1b[2J"), Err(console_error::control));
        assert_eq!(
            writer.write(b"before\x1b]0;new title\x07after"),
            Err(console_error::control)
        );
        // Refused whole: nothing reached the transport, so no half sentence.
        assert_eq!(writer.written(), 0);
        assert!(writer.transport.writes.is_empty());
    }

    #[test]
    fn a_c1_control_cannot_smuggle_an_escape_past_the_filter() {
        // U+009B is CSI: an escape introducer that never contains U+001B, so a
        // filter watching only for the ESC character would pass it. Both ways
        // of presenting it are refused, by different checks.
        let mut writer = console_writer::new(console(), recorder::default()).expect("granted");
        // Encoded as UTF-8, it is well-formed text and the character check
        // catches it.
        assert_eq!(
            writer.write("\u{009b}2J".as_bytes()),
            Err(console_error::control)
        );
        // As a raw byte it is not valid UTF-8 at all, so it never reaches the
        // character check. Refused either way; the reason differs because the
        // fault does.
        assert_eq!(writer.write(b"\x9b2J"), Err(console_error::encoding));
        // NUL and DEL are valid UTF-8 and are control characters.
        assert_eq!(writer.write(b"text\x00more"), Err(console_error::control));
        assert_eq!(writer.write(b"text\x7f"), Err(console_error::control));
        assert_eq!(writer.written(), 0);
    }

    #[test]
    fn lines_and_columns_are_still_text() {
        let mut writer = console_writer::new(console(), recorder::default()).expect("granted");
        writer
            .write(b"name\tvalue\nnext\trow\n")
            .expect("tab and newline are text");
        assert_eq!(writer.written(), 1);
    }

    #[test]
    fn text_must_be_valid_utf8() {
        let mut writer = console_writer::new(console(), recorder::default()).expect("granted");
        assert_eq!(writer.write(b"\xff\xfe"), Err(console_error::encoding));
        writer
            .write("héllo — ok".as_bytes())
            .expect("multi-byte text is text");
    }

    #[test]
    fn a_write_is_bounded_and_an_empty_one_is_distinguishable() {
        let mut writer = console_writer::new(console(), recorder::default()).expect("granted");
        assert_eq!(writer.write(b""), Err(console_error::empty));
        assert_eq!(
            writer.write(&[b'x'; maximum_write_bytes + 1]),
            Err(console_error::length)
        );
        writer
            .write(&[b'x'; maximum_write_bytes])
            .expect("exactly the bound is admitted");
    }

    #[test]
    fn a_handle_granted_for_something_else_cannot_write_here() {
        // A file handle is a real handle. Using it here should fail rather than
        // put a program's output somewhere it was never meant to go.
        let file_like = program_descriptor::try_new(
            0x1000_0000,
            minimum_arena_bytes,
            7,
            334,
            0,
            service::file_read,
            b"",
        )
        .expect("a launch granting only file reading");
        assert!(
            file_like.console().is_none(),
            "no console was granted, so none is offered"
        );
    }

    #[test]
    fn a_partial_or_failed_transport_does_not_count_as_written() {
        let mut short = console_writer::new(
            console(),
            recorder {
                short_by: 1,
                ..Default::default()
            },
        )
        .expect("granted");
        assert_eq!(short.write(b"hello"), Err(console_error::transport));
        assert_eq!(
            short.written(),
            0,
            "a tail silently lost is the failure this prevents"
        );

        let mut failing = console_writer::new(
            console(),
            recorder {
                fail: true,
                ..Default::default()
            },
        )
        .expect("granted");
        assert_eq!(failing.write(b"hello"), Err(console_error::transport));
        assert_eq!(failing.written(), 0);
    }
}
