use super::*;
use serde_json::{Value, json};

struct Memory {
    bytes: Vec<u8>,
    reads: Vec<(u64, usize)>,
    fail_from: Option<u64>,
    short: bool,
}
impl Memory {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            reads: Vec::new(),
            fail_from: None,
            short: false,
        }
    }
}
impl ReadArtifact for Memory {
    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }
    fn read_exact(&mut self, offset: u64, length: usize) -> Result<Vec<u8>, CoreError> {
        self.reads.push((offset, length));
        assert!(length <= diy_ime2_wasm_resource_sdk::MAX_CHUNK_BYTES);
        if self.fail_from.is_some_and(|limit| offset >= limit) {
            return Err(CoreError::new(
                "dictionary-read",
                "injected resource failure",
            ));
        }
        let end = offset as usize + length - usize::from(self.short);
        Ok(self.bytes[offset as usize..end].to_vec())
    }
}
fn config() -> Value {
    json!({"path":"dictionary.tsv"})
}
fn input(value: &str) -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
        text: value.into(),
        meta: None,
    })))
}
fn build(rows: &str) -> Memory {
    Memory::new(
        prepare(
            &config(),
            format!("reading\tsurface\tweight\n{rows}").as_bytes(),
        )
        .unwrap(),
    )
}
fn compare(source: &[u8], queries: &[&str]) -> Memory {
    let old = ExactDictionary::from_tsv_bytes(source).unwrap();
    let mut memory = Memory::new(prepare(&config(), source).unwrap());
    for query in queries {
        let value = input(query);
        assert_eq!(
            lookup(&mut memory, &config(), &value).unwrap(),
            old.lookup_output(&value).unwrap()
        );
    }
    assert_eq!(
        lookup(&mut memory, &config(), &OutputValue::NoOutput).unwrap(),
        OutputValue::NoOutput
    );
    memory
}

#[test]
fn formal_thirteen_cases_keep_outputs_and_errors() {
    let source = include_bytes!("../../../../examples/proposals/dictionary-exact/entries.tsv");
    let cases: Value = serde_json::from_str(include_str!(
        "../../../../tests/contracts/dictionary_exact.json"
    ))
    .unwrap();
    let mut memory = Memory::new(prepare(&config(), source).unwrap());
    let cases = cases["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 13);
    for case in cases {
        let value = if case["input"]["status"] == "no-output" {
            OutputValue::NoOutput
        } else {
            OutputValue::Data(Arc::new(
                serde_json::from_value(case["input"]["packet"].clone()).unwrap(),
            ))
        };
        let before = value.clone();
        match (
            case["expected"]["status"].as_str().unwrap(),
            lookup(&mut memory, &config(), &value),
        ) {
            ("data", Ok(OutputValue::Data(packet))) => assert_eq!(
                packet.as_ref(),
                &serde_json::from_value::<Packet>(case["expected"]["packet"].clone()).unwrap(),
                "{}",
                case["id"]
            ),
            ("no-output", Ok(OutputValue::NoOutput)) => {}
            ("error", Err(error)) => {
                assert_eq!(error.code, case["expected"]["code"], "{}", case["id"])
            }
            (_, result) => panic!("{}: {result:?}", case["id"]),
        }
        assert_eq!(value, before);
    }
}

#[test]
fn collisions_are_resolved_by_full_reading_including_unknown() {
    assert_eq!(hash("ぃぁ"), hash("あだ"));
    assert_eq!(hash("あだ"), hash("ぁみ"));
    compare(
        "reading\tsurface\tweight\nぃぁ\t一\t-0\nあだ\t二\t2\nぃぁ\t三\t-3\n".as_bytes(),
        &["ぃぁ", "あだ", "ぁみ", "", "未知"],
    );
}

#[test]
fn empty_dictionary_bom_endings_unicode_and_order_match_canonical() {
    compare(b"reading\tsurface\tweight", &["", "unknown"]);
    compare(
        "\u{feff}reading\tsurface\tweight\r\na\t𠮷\t-0\nb\t表記\t2\r\na\t後\t-2".as_bytes(),
        &["a", "b", "Ａ", "a\u{301}"],
    );
}

#[test]
fn all_original_source_failures_are_checked_before_returning_an_artifact() {
    let mut sources: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"wrong\theader\tweight".to_vec(),
        b"reading\tsurface\tweight\na\tA\t1\nbad".to_vec(),
        b"reading\tsurface\tweight\na\tA\t1\na\tA\t2".to_vec(),
        b"reading\tsurface\tweight\na\tA\tNaN".to_vec(),
        b"reading\tsurface\tweight\na\tA\t01".to_vec(),
        b"reading\tsurface\tweight\na\tA\t1\n\xff".to_vec(),
        b"reading\tsurface\tweight\na\tA\t1\r".to_vec(),
        b"reading\tsurface\tweight\na\t\0\t1".to_vec(),
        vec![b'x'; MAX_FILE_BYTES + 1],
    ];
    sources.push(
        format!(
            "reading\tsurface\tweight\n{}\tA\t1",
            "a".repeat(MAX_CODE_POINTS + 1)
        )
        .into_bytes(),
    );
    sources.push(
        format!(
            "reading\tsurface\tweight\na\t{}\t1",
            "x".repeat(MAX_LINE_BYTES + 1)
        )
        .into_bytes(),
    );
    sources.push(
        format!(
            "reading\tsurface\tweight\n{}",
            (0..=MAX_CANDIDATES)
                .map(|i| format!("a\t{i}\t1\n"))
                .collect::<String>()
        )
        .into_bytes(),
    );
    sources.push(
        format!(
            "reading\tsurface\tweight\n{}",
            (0..=MAX_ROWS)
                .map(|i| format!("{i}\tA\t1\n"))
                .collect::<String>()
        )
        .into_bytes(),
    );
    for source in sources {
        let expected = ExactDictionary::from_tsv_bytes(&source).unwrap_err();
        assert_eq!(prepare(&config(), &source).unwrap_err().code, expected.code);
    }
    for cfg in [
        json!({}),
        json!({"path":""}),
        json!({"path":"dictionary.tsv","extra":true}),
    ] {
        assert_eq!(
            prepare(&cfg, b"reading\tsurface\tweight").unwrap_err().code,
            "invalid-config"
        );
    }
}

#[test]
fn invalid_header_config_offsets_and_candidate_bytes_are_errors() {
    let original = build("a\tA\t1").bytes;
    let slot = HEADER_BYTES + "dictionary.tsv".len();
    let candidate = u64_at(&original, slot + 20).unwrap() as usize;
    let mut mutations = Vec::new();
    for (offset, value) in [(0, 0), (8, 2), (12, 0), (16, 0), (20, 0), (24, 0)] {
        let mut bytes = original.clone();
        bytes[offset] = value;
        mutations.push(bytes);
    }
    let mut bytes = original.clone();
    bytes[slot + 8..slot + 16].copy_from_slice(&u64::MAX.to_le_bytes());
    mutations.push(bytes);
    let mut bytes = original.clone();
    bytes[slot + 20..slot + 28].copy_from_slice(&0u64.to_le_bytes());
    mutations.push(bytes);
    let mut bytes = original.clone();
    bytes[slot + 28..slot + 32].copy_from_slice(&u32::MAX.to_le_bytes());
    mutations.push(bytes);
    let mut bytes = original.clone();
    bytes[candidate..candidate + 4].copy_from_slice(&0u32.to_le_bytes());
    mutations.push(bytes);
    let mut bytes = original.clone();
    bytes[candidate + 8] = 0xff;
    mutations.push(bytes);
    let mut bytes = original.clone();
    bytes[candidate + 9..candidate + 17].copy_from_slice(&f64::NAN.to_le_bytes());
    mutations.push(bytes);
    let mut bytes = original.clone();
    bytes.pop();
    mutations.push(bytes);
    let mut bytes = original.clone();
    bytes.push(0);
    let len = bytes.len() as u64;
    bytes[24..32].copy_from_slice(&len.to_le_bytes());
    bytes[slot + 28..slot + 32].copy_from_slice(&18u32.to_le_bytes());
    mutations.push(bytes);
    for bytes in mutations {
        assert!(lookup(&mut Memory::new(bytes), &config(), &input("a")).is_err());
    }
    assert_eq!(
        lookup(
            &mut Memory::new(original),
            &json!({"path":"other.tsv"}),
            &input("a")
        )
        .unwrap_err()
        .code,
        "dictionary-not-prepared"
    );
}

#[test]
fn maximum_unicode_candidates_survive_pages_without_truncation() {
    let rows = (0..MAX_CANDIDATES)
        .map(|i| {
            format!(
                "a\t{}{}\t{i}\n",
                "漢".repeat(MAX_CODE_POINTS - 3),
                format_args!("{i:03}")
            )
        })
        .collect::<String>();
    let source = format!("reading\tsurface\tweight\n{rows}");
    assert!(source.len() < MAX_FILE_BYTES);
    let mut memory = compare(source.as_bytes(), &["a"]);
    let slot = HEADER_BYTES + "dictionary.tsv".len();
    let candidate = u64_at(&memory.bytes, slot + 20).unwrap();
    assert!(
        u32_at(&memory.bytes, slot + 28).unwrap() as usize
            > diy_ime2_wasm_resource_sdk::MAX_CHUNK_BYTES
    );
    memory.fail_from = Some(candidate + diy_ime2_wasm_resource_sdk::MAX_CHUNK_BYTES as u64);
    assert_eq!(
        lookup(&mut memory, &config(), &input("a"))
            .unwrap_err()
            .code,
        "dictionary-read"
    );
}

#[test]
fn short_reads_and_invalid_input_do_not_become_unknown_words() {
    let mut memory = build("a\tA\t1");
    memory.short = true;
    assert!(lookup(&mut memory, &config(), &input("a")).is_err());
    let mut memory = build("a\tA\t1");
    let oversized = input(&"a".repeat(MAX_CODE_POINTS + 1));
    let expected = ExactDictionary::from_tsv_bytes(b"reading\tsurface\tweight")
        .unwrap()
        .lookup_output(&oversized)
        .unwrap_err();
    assert_eq!(
        lookup(&mut memory, &config(), &oversized).unwrap_err().code,
        expected.code
    );
}
