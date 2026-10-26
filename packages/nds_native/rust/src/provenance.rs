const PREFIX: &[u8; 13] = b"NDS-SOURCE-1\0";

const fn record() -> [u8; 54] {
    let source = match option_env!("NDS_SOURCE_COMMIT") {
        Some(value) => value.as_bytes(),
        None => b"unknown",
    };
    assert!(source.len() == 40 || source.len() == 7);
    let mut output = [0; 54];
    let mut index = 0;
    while index < PREFIX.len() {
        output[index] = PREFIX[index];
        index += 1;
    }
    index = 0;
    while index < source.len() {
        let byte = source[index];
        assert!(if source.len() == 7 {
            byte == b"unknown"[index]
        } else {
            byte >= b'0' && byte <= b'9' || byte >= b'a' && byte <= b'f'
        });
        output[PREFIX.len() + index] = byte;
        index += 1;
    }
    output
}

// SDK metadata and artifact verification consume this same retained record.
#[used]
#[unsafe(no_mangle)]
pub static NDS_NATIVE_PROVENANCE: [u8; 54] = record();

pub(crate) fn source_commit() -> &'static str {
    let value = &NDS_NATIVE_PROVENANCE[PREFIX.len()..];
    let length = value.iter().position(|byte| *byte == 0).unwrap();
    std::str::from_utf8(&value[..length]).expect("compile-time validated source SHA")
}

/// The assembly supplies its locked standards identity at compile time.
/// The owning telemetry SDK validates a supplied nonempty value.
pub(crate) fn standards_release() -> &'static str {
    match option_env!("NDS_STANDARDS_RELEASE") {
        Some(value) if !value.is_empty() => value,
        _ => "unreleased",
    }
}
