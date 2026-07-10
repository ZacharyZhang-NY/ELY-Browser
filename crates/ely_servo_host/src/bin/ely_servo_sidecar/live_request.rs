use std::io::{self, BufRead, Read};

pub(super) use ely_servo_host::MAX_REQUEST_LINE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestLineRead {
    Eof,
    Ready,
    TooLarge,
}

pub(super) fn read_request_line(
    reader: &mut impl BufRead,
    line: &mut Vec<u8>,
) -> io::Result<RequestLineRead> {
    line.clear();
    let bytes = {
        let mut limited = Read::by_ref(reader).take((MAX_REQUEST_LINE_BYTES + 1) as u64);
        limited.read_until(b'\n', line)?
    };
    if bytes == 0 {
        return Ok(RequestLineRead::Eof);
    }
    if bytes <= MAX_REQUEST_LINE_BYTES {
        return Ok(RequestLineRead::Ready);
    }
    if !line.ends_with(b"\n") {
        reader.skip_until(b'\n')?;
    }
    Ok(RequestLineRead::TooLarge)
}
