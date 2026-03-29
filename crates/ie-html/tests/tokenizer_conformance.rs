use std::collections::HashMap;
use std::path::PathBuf;

use ie_html::token::Token;
use ie_html::tokenizer::{Tokenizer, TokenizerState};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/html5lib-tokenizer")
}

#[derive(Debug)]
struct TestCase {
    description: String,
    input: String,
    expected: Vec<ExpectedToken>,
    initial_states: Vec<String>,
    last_start_tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum ExpectedToken {
    Doctype {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
        correctness: bool,
    },
    StartTag {
        name: String,
        attrs: HashMap<String, String>,
        self_closing: bool,
    },
    EndTag {
        name: String,
    },
    Character(String),
    Comment(String),
}

fn parse_test_file(path: &std::path::Path) -> Vec<TestCase> {
    let content = std::fs::read_to_string(path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    let tests = json["tests"].as_array().unwrap();

    tests
        .iter()
        .filter_map(|test| {
            // Skip "doubleEscaped" tests — they need special input decoding
            if test
                .get("doubleEscaped")
                .is_some_and(|v| v.as_bool() == Some(true))
            {
                return None;
            }

            let description = test["description"].as_str().unwrap().to_string();
            let input = test["input"].as_str().unwrap().to_string();
            let initial_states = test
                .get("initialStates")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect()
                })
                .unwrap_or_else(|| vec!["Data state".to_string()]);
            let last_start_tag = test
                .get("lastStartTag")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let output = test["output"].as_array().unwrap();
            let expected = output
                .iter()
                .map(|tok| {
                    let arr = tok.as_array().unwrap();
                    let token_type = arr[0].as_str().unwrap();
                    match token_type {
                        "DOCTYPE" => ExpectedToken::Doctype {
                            name: arr[1].as_str().map(|s| s.to_string()),
                            public_id: arr[2].as_str().map(|s| s.to_string()),
                            system_id: arr[3].as_str().map(|s| s.to_string()),
                            correctness: arr[4].as_bool().unwrap(),
                        },
                        "StartTag" => {
                            let name = arr[1].as_str().unwrap().to_string();
                            let attrs_obj = arr[2].as_object().unwrap();
                            let attrs: HashMap<String, String> = attrs_obj
                                .iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
                                .collect();
                            let self_closing = arr.get(3).is_some_and(|v| v == true);
                            ExpectedToken::StartTag {
                                name,
                                attrs,
                                self_closing,
                            }
                        }
                        "EndTag" => ExpectedToken::EndTag {
                            name: arr[1].as_str().unwrap().to_string(),
                        },
                        "Character" => {
                            ExpectedToken::Character(arr[1].as_str().unwrap().to_string())
                        }
                        "Comment" => ExpectedToken::Comment(arr[1].as_str().unwrap().to_string()),
                        other => panic!("unknown token type: {other}"),
                    }
                })
                .collect();

            Some(TestCase {
                description,
                input,
                expected,
                initial_states,
                last_start_tag,
            })
        })
        .collect()
}

fn map_initial_state(state: &str) -> TokenizerState {
    match state {
        "Data state" => TokenizerState::Data,
        "RCDATA state" => TokenizerState::RcData,
        "RAWTEXT state" => TokenizerState::RawText,
        "Script data state" => TokenizerState::ScriptData,
        "PLAINTEXT state" => TokenizerState::PlainText,
        "CDATA section state" => TokenizerState::CDataSection,
        _ => panic!("unknown initial state: {state}"),
    }
}

/// Convert our Token stream to ExpectedToken list (coalescing adjacent Characters)
fn coalesce_tokens(tokens: Vec<Token>) -> Vec<ExpectedToken> {
    let mut result = Vec::new();
    let mut char_buf = String::new();

    for token in tokens {
        match token {
            Token::Character(c) => char_buf.push(c),
            Token::Eof => {
                if !char_buf.is_empty() {
                    result.push(ExpectedToken::Character(std::mem::take(&mut char_buf)));
                }
                // Eof is not in html5lib output
            }
            other => {
                if !char_buf.is_empty() {
                    result.push(ExpectedToken::Character(std::mem::take(&mut char_buf)));
                }
                result.push(match other {
                    Token::Doctype {
                        name,
                        public_id,
                        system_id,
                        force_quirks,
                    } => ExpectedToken::Doctype {
                        name,
                        public_id,
                        system_id,
                        correctness: !force_quirks,
                    },
                    Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    } => {
                        let attrs: HashMap<String, String> =
                            attributes.into_iter().map(|a| (a.name, a.value)).collect();
                        ExpectedToken::StartTag {
                            name,
                            attrs,
                            self_closing,
                        }
                    }
                    Token::EndTag { name } => ExpectedToken::EndTag { name },
                    Token::Comment(text) => ExpectedToken::Comment(text),
                    _ => unreachable!(),
                });
            }
        }
    }

    if !char_buf.is_empty() {
        result.push(ExpectedToken::Character(char_buf));
    }

    result
}

fn run_test_file(filename: &str) -> (usize, usize, Vec<String>) {
    let path = fixtures_dir().join(filename);
    let tests = parse_test_file(&path);
    let mut total = 0;
    let mut passed = 0;
    let mut failures = Vec::new();

    for test in &tests {
        for state_name in &test.initial_states {
            total += 1;
            let state = map_initial_state(state_name);

            let mut tokenizer = Tokenizer::new(&test.input);
            tokenizer.set_state(state);
            if let Some(ref tag) = test.last_start_tag {
                tokenizer.set_last_start_tag(tag);
            }

            let tokens: Vec<Token> = tokenizer.collect();
            let actual = coalesce_tokens(tokens);

            if actual == test.expected {
                passed += 1;
            } else {
                let desc = if test.initial_states.len() > 1 {
                    format!("{} [{}]", test.description, state_name)
                } else {
                    test.description.clone()
                };
                failures.push(format!(
                    "FAIL: {desc}\n  input: {:?}\n  expected: {expected:?}\n  actual:   {actual:?}",
                    test.input,
                    expected = test.expected,
                ));
            }
        }
    }

    (total, passed, failures)
}

#[test]
fn html5lib_conformance() {
    // namedEntities.test excluded: 42K lines / 4210 tests causes OOM.
    // Named entity lookup is covered by unit tests in entities.rs.
    let test_files = [
        "test1.test",
        "test2.test",
        "test3.test",
        "test4.test",
        "numericEntities.test",
        "unicodeChars.test",
        "unicodeCharsProblematic.test",
        "contentModelFlags.test",
        "escapeFlag.test",
        "domjs.test",
        "pendingSpecChanges.test",
        "entities.test",
        "namedEntities.test",
    ];

    let mut grand_total = 0;
    let mut grand_passed = 0;
    let mut grand_failures = 0;

    for filename in &test_files {
        let (total, passed, failures) = run_test_file(filename);
        let fail_count = failures.len();
        if fail_count > 0 {
            eprintln!("\n--- {filename}: {passed}/{total} passed ---");
            for f in &failures[..fail_count.min(3)] {
                eprintln!("  {f}");
            }
            if fail_count > 3 {
                eprintln!("  ... and {} more", fail_count - 3);
            }
        } else {
            eprintln!("{filename}: {passed}/{total} passed");
        }
        grand_total += total;
        // Only keep count, don't accumulate full failure strings
        grand_passed += passed;
        grand_failures += fail_count;
    }

    let pass_rate = (grand_passed as f64 / grand_total as f64) * 100.0;
    eprintln!("\n=== TOTAL: {grand_passed}/{grand_total} passed ({pass_rate:.1}%) ===");

    // Require >98% pass rate
    assert!(
        pass_rate >= 98.0,
        "html5lib conformance {pass_rate:.1}% < 98% ({grand_failures} failures out of {grand_total})",
    );
}
