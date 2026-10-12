use std::io::{self, BufRead, BufReader, Cursor, Read, Write};

use ktlsp_protocol::{MAX_HEADER_BYTES, MAX_PAYLOAD_BYTES, ProtocolError, read_frame, write_frame};

fn read(bytes: &[u8]) -> Result<Option<Vec<u8>>, ProtocolError> {
    read_frame(&mut Cursor::new(bytes))
}

#[test]
fn fragmented_and_back_to_back_frames_preserve_bytes() {
    let payloads = [
        "{\"text\":\"λ😀\"}".as_bytes(),
        b"".as_slice(),
        b"\0\xff\r\n\r\n",
    ];
    let mut wire = Vec::new();
    for payload in payloads {
        write_frame(&mut wire, payload).unwrap();
    }
    // Every small buffer size splits different positions in the delimiter/body.
    for capacity in 1..=32 {
        let mut reader = BufReader::with_capacity(capacity, Cursor::new(&wire));
        for payload in payloads {
            assert_eq!(read_frame(&mut reader).unwrap().unwrap(), payload);
        }
        assert!(read_frame(&mut reader).unwrap().is_none());
    }
    assert!(wire.starts_with(b"Content-Length: 17\r\n\r\n"));
}

#[test]
fn accepts_extra_headers_case_and_ascii_whitespace() {
    assert_eq!(
        read(b"X-Trace: arbitrary: value\r\ncOnTeNt-LeNgTh:\t 0003 \t\r\nContent-Type: application/vscode-jsonrpc; CHARSET=\"UtF8\"\r\n\r\nabc")
            .unwrap(),
        Some(b"abc".to_vec())
    );
    assert_eq!(
        read(b"Content-Type: text/plain\r\nContent-Length: 0\r\n\r\n").unwrap(),
        Some(vec![])
    );
    assert_eq!(read(b"Content-Length: 0\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8; version=2\r\n\r\n").unwrap(), Some(vec![]));
}

#[test]
fn validates_length_before_body_read_or_allocation() {
    for value in [
        "",
        "-1",
        "+1",
        "1.0",
        "1 0",
        "0x10",
        "NaN",
        "184467440737095516160",
    ] {
        let wire = format!("Content-Length: {value}\r\n\r\n");
        assert!(
            matches!(
                read(wire.as_bytes()),
                Err(ProtocolError::InvalidContentLength)
            ),
            "{value:?}"
        );
    }
    assert!(matches!(
        read(b"X-Other: 1\r\n\r\n"),
        Err(ProtocolError::MissingContentLength)
    ));
    assert!(matches!(
        read(b"Content-Length: 1\r\ncontent-length: 1\r\n\r\nx"),
        Err(ProtocolError::DuplicateContentLength)
    ));
    let wire = format!("Content-Length: {}\r\n\r\n", MAX_PAYLOAD_BYTES + 1);
    assert!(matches!(
        read(wire.as_bytes()),
        Err(ProtocolError::PayloadTooLarge)
    ));
}

#[test]
fn rejects_invalid_header_grammar() {
    for wire in [
        b"Content-Length: 0\nX: a\r\n\r\n".as_slice(),
        b"Content-Length: 0\r\n\nX: a\r\n\r\n",
        b"Content-Length: 0\rX: a\r\n\r\n",
        b"Content-Length : 0\r\n\r\n",
        b" Content-Length: 0\r\n\r\n",
        b"Content-Length: 0\r\n continuation\r\n\r\n",
        b": value\r\nContent-Length: 0\r\n\r\n",
        b"NoColon\r\nContent-Length: 0\r\n\r\n",
        b"Content-Length: 0\r\nX: \xff\r\n\r\n",
        b"Content-Length: 0\r\nX: \0\r\n\r\n",
        b"Content-Length: 0\r\nX: \x7f\r\n\r\n",
        b"\r\n\r\n",
    ] {
        assert!(
            matches!(read(wire), Err(ProtocolError::InvalidHeader)),
            "{wire:?}"
        );
    }
}

#[test]
fn validates_content_type_without_inspecting_payload() {
    for value in [
        "application/vscode-jsonrpc; charset=ascii",
        "application/vscode-jsonrpc; charset=UTF-16",
    ] {
        let wire = format!("Content-Length: 0\r\nContent-Type: {value}\r\n\r\n");
        assert!(matches!(
            read(wire.as_bytes()),
            Err(ProtocolError::UnsupportedCharset)
        ));
    }
    for value in [
        "",
        "invalid",
        "/json",
        "application/",
        "application/json;",
        "application/json; charset",
        "application/json; charset=",
        "application/json; charset=utf-8; Charset=utf8",
        "application/json; charset=\"utf-8",
        "application/json; charset=utf-8\"",
    ] {
        let wire = format!("Content-Length: 0\r\nContent-Type: {value}\r\n\r\n");
        assert!(
            matches!(
                read(wire.as_bytes()),
                Err(ProtocolError::InvalidContentType)
            ),
            "{value:?}"
        );
    }
    assert!(matches!(
        read(b"Content-Length: 0\r\nContent-Type: text/plain\r\nContent-Type: text/plain\r\n\r\n"),
        Err(ProtocolError::InvalidContentType)
    ));
}

#[test]
fn header_limit_counts_the_delimiter_and_stops_at_cap() {
    let prefix = b"Content-Length: 0\r\nX: ";
    let mut wire = prefix.to_vec();
    wire.resize(MAX_HEADER_BYTES - 4, b'a');
    wire.extend_from_slice(b"\r\n\r\n");
    assert_eq!(read(&wire).unwrap(), Some(vec![]));
    wire.insert(prefix.len(), b'a');
    let mut cursor = Cursor::new(&wire);
    assert!(matches!(
        read_frame(&mut cursor),
        Err(ProtocolError::HeaderTooLarge)
    ));
    assert_eq!(cursor.position(), MAX_HEADER_BYTES as u64);
    assert!(matches!(
        read(&vec![b'a'; MAX_HEADER_BYTES * 2]),
        Err(ProtocolError::HeaderTooLarge)
    ));
}

#[test]
fn payload_limit_is_inclusive_and_writer_rejects_before_output() {
    let mut payload = vec![b'a'; MAX_PAYLOAD_BYTES];
    let mut wire = Vec::new();
    write_frame(&mut wire, &payload).unwrap();
    assert_eq!(read(&wire).unwrap().unwrap(), payload);
    payload.push(b'a');
    let mut untouched = vec![9];
    assert!(matches!(
        write_frame(&mut untouched, &payload),
        Err(ProtocolError::PayloadTooLarge)
    ));
    assert_eq!(untouched, [9]);
}

#[test]
fn eof_is_clean_only_before_any_frame_bytes() {
    assert_eq!(read(b"").unwrap(), None);
    let wire = b"Content-Length: 3\r\n\r\nabc";
    let header_end = wire.len() - 3;
    for end in 1..wire.len() {
        let result = read(&wire[..end]);
        if end < header_end {
            assert!(
                matches!(result, Err(ProtocolError::TruncatedHeader)),
                "{end}"
            );
        } else {
            assert!(
                matches!(result, Err(ProtocolError::TruncatedPayload)),
                "{end}"
            );
        }
    }
    // LF-only framing is incomplete, not a successfully guessed frame.
    assert!(matches!(
        read(b"Content-Length: 0\n\n"),
        Err(ProtocolError::TruncatedHeader)
    ));
}

struct InterruptedReader {
    cursor: Cursor<Vec<u8>>,
    interrupt: bool,
}

impl Read for InterruptedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if std::mem::replace(&mut self.interrupt, false) {
            Err(io::ErrorKind::Interrupted.into())
        } else {
            self.cursor.read(buffer)
        }
    }
}

impl BufRead for InterruptedReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if std::mem::replace(&mut self.interrupt, false) {
            Err(io::ErrorKind::Interrupted.into())
        } else {
            self.cursor.fill_buf()
        }
    }

    fn consume(&mut self, amount: usize) {
        self.cursor.consume(amount);
        self.interrupt = true;
    }
}

#[test]
fn retries_interrupted_io_and_handles_short_writes() {
    let mut reader = InterruptedReader {
        cursor: Cursor::new(b"Content-Length: 3\r\n\r\nabc".to_vec()),
        interrupt: true,
    };
    assert_eq!(read_frame(&mut reader).unwrap(), Some(b"abc".to_vec()));
    struct ShortWriter {
        bytes: Vec<u8>,
        interrupt: bool,
    }
    impl Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if std::mem::replace(&mut self.interrupt, false) {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let amount = bytes.len().min(1);
            self.bytes.extend_from_slice(&bytes[..amount]);
            self.interrupt = true;
            Ok(amount)
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("writer must not flush")
        }
    }
    let mut writer = ShortWriter {
        bytes: vec![],
        interrupt: true,
    };
    write_frame(&mut writer, "😀".as_bytes()).unwrap();
    assert_eq!(writer.bytes, b"Content-Length: 4\r\n\r\n\xf0\x9f\x98\x80");
}

#[test]
fn propagates_read_and_write_errors() {
    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    impl BufRead for BrokenReader {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn consume(&mut self, _: usize) {}
    }
    assert!(
        matches!(read_frame(&mut BrokenReader), Err(ProtocolError::Io(e)) if e.kind() == io::ErrorKind::BrokenPipe)
    );
    struct BrokenBody(Cursor<Vec<u8>>);
    impl Read for BrokenBody {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::ConnectionReset.into())
        }
    }
    impl BufRead for BrokenBody {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            self.0.fill_buf()
        }
        fn consume(&mut self, amount: usize) {
            self.0.consume(amount);
        }
    }
    assert!(
        matches!(read_frame(&mut BrokenBody(Cursor::new(b"Content-Length: 1\r\n\r\n".to_vec()))), Err(ProtocolError::Io(e)) if e.kind() == io::ErrorKind::ConnectionReset)
    );
    struct BrokenWriter {
        remaining: usize,
    }
    impl Write for BrokenWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            let amount = bytes.len().min(self.remaining);
            self.remaining -= amount;
            Ok(amount)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for remaining in [0, 5, 21, 22, 23] {
        assert!(
            matches!(write_frame(&mut BrokenWriter { remaining }, b"abc"), Err(ProtocolError::Io(e)) if e.kind() == io::ErrorKind::BrokenPipe)
        );
    }
    struct ZeroWriter;
    impl Write for ZeroWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Ok(0)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(
        matches!(write_frame(&mut ZeroWriter, b"abc"), Err(ProtocolError::Io(e)) if e.kind() == io::ErrorKind::WriteZero)
    );
}

#[test]
fn deterministic_adversarial_corpus_never_panics() {
    let mut seed = 0x7d43_15abu32;
    for size in 0..512 {
        let mut bytes = Vec::with_capacity(size + 4);
        for _ in 0..size {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            bytes.push((seed >> 24) as u8);
        }
        bytes.extend_from_slice(b"\r\n\r\n");
        for capacity in [1, 7, 32] {
            let mut reader = BufReader::with_capacity(capacity, Cursor::new(&bytes));
            assert!(read_frame(&mut reader).is_err());
        }
    }
}
