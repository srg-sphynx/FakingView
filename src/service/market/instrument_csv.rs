use crate::service::ProviderError;

pub(super) const MAX_COLUMNS: usize = 64;
pub(super) const MAX_FIELD_BYTES: usize = 16 * 1024;
const MAX_RECORD_BYTES: usize = 128 * 1024;

/// RFC 4180 records, including escaped quotes, CRLF and embedded newlines. Parsing records as
/// they arrive avoids a second copy of the entire (large) instrument directory.
pub(super) struct Records<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Records<'a> {
    pub(super) fn new(body: &'a str) -> Self {
        Self {
            bytes: body.trim_start_matches('\u{feff}').as_bytes(),
            offset: 0,
        }
    }

    pub(super) fn next(&mut self) -> Result<Option<Vec<String>>, ProviderError> {
        if self.offset >= self.bytes.len() {
            return Ok(None);
        }
        let mut fields = Vec::new();
        let mut field = Vec::new();
        let start = self.offset;
        let mut quoted = false;
        let mut closed = false;
        while let Some(&byte) = self.bytes.get(self.offset) {
            self.offset += 1;
            if self.offset - start > MAX_RECORD_BYTES || field.len() > MAX_FIELD_BYTES {
                return Err(invalid_csv());
            }
            if quoted {
                if byte == b'"' {
                    if self.bytes.get(self.offset) == Some(&b'"') {
                        self.offset += 1;
                        field.push(b'"');
                    } else {
                        quoted = false;
                        closed = true;
                    }
                } else {
                    field.push(byte);
                }
                continue;
            }
            match byte {
                b'"' if field.is_empty() && !closed => quoted = true,
                b',' => {
                    push_field(&mut fields, std::mem::take(&mut field))?;
                    closed = false;
                }
                b'\n' | b'\r' => {
                    if byte == b'\r' && self.bytes.get(self.offset) == Some(&b'\n') {
                        self.offset += 1;
                    }
                    push_field(&mut fields, field)?;
                    return Ok(Some(fields));
                }
                b'"' => return Err(invalid_csv()),
                _ if closed => return Err(invalid_csv()),
                _ => field.push(byte),
            }
        }
        if quoted {
            return Err(invalid_csv());
        }
        push_field(&mut fields, field)?;
        Ok(Some(fields))
    }
}

fn invalid_csv() -> ProviderError {
    ProviderError::data("Malformed instrument CSV")
}
fn text(bytes: Vec<u8>) -> Result<String, ProviderError> {
    String::from_utf8(bytes).map_err(|_| invalid_csv())
}

fn push_field(fields: &mut Vec<String>, field: Vec<u8>) -> Result<(), ProviderError> {
    if fields.len() >= MAX_COLUMNS || field.len() > MAX_FIELD_BYTES {
        return Err(invalid_csv());
    }
    fields.push(text(field)?);
    Ok(())
}
