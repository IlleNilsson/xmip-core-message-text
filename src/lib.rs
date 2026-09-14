#![forbid(unsafe_code)]

//! Text: content that reads as UTF-8 and carries no NUL byte. One part, the
//! whole Stream, with the media type it came with or `text/plain` when it
//! came with none; no announced type, because a text says nothing about
//! itself (ADR-0047).
//!
//! The shape refuses what is not text — a byte sequence that is not UTF-8,
//! or a NUL, which no text carries and which is the surest sign of bytes that
//! are only bytes — and says at which byte it stopped reading. Encoding is
//! not converted here; a Stream in another encoding is a transport's or a
//! transformation's to bring to UTF-8. The check is the Foundation's
//! [`record::text`], the one `csv` and `fixed-width` make (ADR-0044).

use message::record;
use message::{Part, Shape, ShapeError, Shaped};
use stream::Stream;

/// The text shape.
#[derive(Clone, Copy, Debug, Default)]
pub struct Text;

/// The media types a text claims. Any other `text/*` is a text too, but a
/// shape claims by list and `choose` believes the list.
const MEDIA_TYPES: &[&str] = &[
    "text/plain",
    "text/html",
    "text/markdown",
    "text/css",
    "text/javascript",
    "text/x-log",
];

impl Shape for Text {
    fn technology(&self) -> &'static str {
        "text"
    }

    fn media_types(&self) -> &'static [&'static str] {
        MEDIA_TYPES
    }

    fn recognises(&self, bytes: &[u8]) -> bool {
        record::text(bytes).is_ok()
    }

    fn shape(&self, stream: &Stream) -> Result<Shaped, ShapeError> {
        record::text(stream.bytes()).map_err(|stop| ShapeError::refused("text", stop))?;
        let media = stream
            .media_type()
            .map_or_else(|| "text/plain".to_string(), str::to_string);
        Ok(Shaped {
            parts: vec![Part::new(None, stream.bytes(), Some(media))],
            message_type: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xcore::StreamId;

    fn stream(bytes: &[u8], media: Option<&str>) -> Stream {
        Stream::new(StreamId::new(1), bytes.to_vec(), media.map(str::to_string))
    }

    #[test]
    fn utf8_text_is_one_part_that_is_plain_text_unless_told_otherwise() {
        let shaped = Text
            .shape(&stream("grüß dich\n".as_bytes(), None))
            .expect("text");
        assert_eq!(shaped.parts.len(), 1);
        assert_eq!(shaped.parts[0].name, None);
        assert_eq!(shaped.parts[0].bytes, "grüß dich\n".as_bytes());
        assert_eq!(shaped.parts[0].media_type.as_deref(), Some("text/plain"));
        assert_eq!(shaped.message_type, None);

        let html = Text
            .shape(&stream(b"<p>x</p>", Some("text/html; charset=utf-8")))
            .expect("html is text");
        assert_eq!(
            html.parts[0].media_type.as_deref(),
            Some("text/html; charset=utf-8")
        );
    }

    #[test]
    fn bytes_that_are_not_utf8_are_refused_where_the_text_stops() {
        let refused = Text
            .shape(&stream(b"abc\xff\xfe", None))
            .expect_err("not UTF-8");
        assert_eq!(refused.offset, Some(3));
        assert_eq!(refused.to_string(), "text: not UTF-8 at byte 3");

        let nul = Text
            .shape(&stream(b"ab\x00cd", None))
            .expect_err("a NUL is not text");
        assert_eq!(nul.offset, Some(2));
        assert_eq!(nul.reason, "a NUL byte");
    }

    #[test]
    fn the_shape_claims_plain_text_and_recognises_utf8_without_nul() {
        assert_eq!(Text.technology(), "text");
        assert!(Text.media_types().contains(&"text/plain"));
        assert!(Text.media_types().contains(&"text/html"));
        assert!(Text.recognises(b"hello"));
        assert!(Text.recognises("ünïcödé".as_bytes()));
        assert!(!Text.recognises(b"\x80"));
        assert!(!Text.recognises(b"a\x00b"));
    }

    #[test]
    fn choose_takes_text_by_media_type_or_by_look_before_a_shape_after_it() {
        struct Everything;
        impl Shape for Everything {
            fn technology(&self) -> &'static str {
                "everything"
            }
            fn media_types(&self) -> &'static [&'static str] {
                &[]
            }
            fn recognises(&self, _: &[u8]) -> bool {
                true
            }
            fn shape(&self, stream: &Stream) -> Result<Shaped, ShapeError> {
                Ok(Shaped {
                    parts: vec![Part::whole(stream)],
                    message_type: None,
                })
            }
        }
        let shapes: [&dyn Shape; 2] = [&Text, &Everything];
        let by_media = message::choose(&shapes, &stream(b"\xff", Some("Text/Plain")));
        assert_eq!(by_media.map(Shape::technology), Some("text"));
        let by_look = message::choose(&shapes, &stream(b"plain words", None));
        assert_eq!(by_look.map(Shape::technology), Some("text"));
        let not_text = message::choose(&shapes, &stream(b"\x00\x01", None));
        assert_eq!(not_text.map(Shape::technology), Some("everything"));
    }
}
