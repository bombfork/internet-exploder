use std::path::PathBuf;

use ie_dom::{Document, NodeId, NodeKind};
use ie_html::parse;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/html5lib-tree")
}

#[derive(Debug)]
struct TreeTest {
    description: String,
    input: String,
    expected_tree: String,
    script_on: bool,
    is_fragment: bool,
}

fn parse_test_file(path: &std::path::Path) -> Vec<TreeTest> {
    let content = std::fs::read_to_string(path).unwrap();
    let mut tests = Vec::new();
    let mut lines = content.lines().peekable();

    while lines.peek().is_some() {
        // Skip until #data
        loop {
            match lines.peek() {
                Some(&"#data") => {
                    lines.next();
                    break;
                }
                Some(_) => {
                    lines.next();
                }
                None => return tests,
            }
        }

        // Read input until next section marker
        let mut input_lines = Vec::new();
        while let Some(&line) = lines.peek() {
            if line.starts_with('#') {
                break;
            }
            input_lines.push(line);
            lines.next();
        }
        let input = input_lines.join("\n");

        // Read remaining sections
        let mut expected_tree = String::new();
        let mut script_on = false;
        let mut is_fragment = false;

        while let Some(&line) = lines.peek() {
            if line == "#data" {
                break; // next test
            }
            match line {
                "#errors" | "#new-errors" => {
                    lines.next();
                    // Skip error lines
                    while let Some(&l) = lines.peek() {
                        if l.starts_with('#') || l.is_empty() {
                            // Empty line between sections or next section
                            if l.is_empty() {
                                lines.next();
                                continue;
                            }
                            break;
                        }
                        lines.next();
                    }
                }
                "#script-on" => {
                    script_on = true;
                    lines.next();
                }
                "#script-off" => {
                    script_on = false;
                    lines.next();
                }
                "#document-fragment" => {
                    is_fragment = true;
                    lines.next();
                    // Skip the fragment context element line
                    lines.next();
                }
                "#document" => {
                    lines.next();
                    let mut tree_lines = Vec::new();
                    while let Some(&l) = lines.peek() {
                        if !l.starts_with("| ") && !l.is_empty() {
                            break;
                        }
                        if l.is_empty() {
                            // Could be blank line in test output or separator
                            // Peek ahead to see if next line continues the tree
                            tree_lines.push(l);
                            lines.next();
                            continue;
                        }
                        tree_lines.push(l);
                        lines.next();
                    }
                    // Trim trailing empty lines
                    while tree_lines.last() == Some(&"") {
                        tree_lines.pop();
                    }
                    expected_tree = tree_lines.join("\n");
                }
                _ => {
                    lines.next();
                }
            }
        }

        if !expected_tree.is_empty() {
            let desc = if input.len() > 60 {
                format!("{}...", &input[..60])
            } else {
                input.clone()
            };
            tests.push(TreeTest {
                description: desc,
                input,
                expected_tree,
                script_on,
                is_fragment,
            });
        }
    }

    tests
}

/// Serialize a DOM document to the html5lib tree test format.
fn serialize_tree_with_doctype(result: &ie_html::ParseResult) -> String {
    let mut output = String::new();
    // Output doctype if present
    if result.doctype_name.is_some()
        || result.doctype_public_id.is_some()
        || result.doctype_system_id.is_some()
    {
        let name = result.doctype_name.as_deref().unwrap_or("");
        let public = result.doctype_public_id.as_deref();
        let system = result.doctype_system_id.as_deref();
        match (public, system) {
            (Some(p), Some(s)) => {
                output.push_str(&format!("| <!DOCTYPE {name} \"{p}\" \"{s}\">\n"))
            }
            (Some(p), None) => output.push_str(&format!("| <!DOCTYPE {name} \"{p}\" \"\">\n")),
            (None, Some(s)) => output.push_str(&format!("| <!DOCTYPE {name} \"\" \"{s}\">\n")),
            (None, None) => output.push_str(&format!("| <!DOCTYPE {name}>\n")),
        }
    }
    let doc = &result.document;
    let root = doc.root;
    let root_node = doc.node(root).unwrap();
    for &child_id in &root_node.children {
        serialize_node(doc, child_id, 0, &mut output);
    }
    // Trim trailing newline
    if output.ends_with('\n') {
        output.pop();
    }
    output
}

fn serialize_node(doc: &Document, id: NodeId, depth: usize, output: &mut String) {
    let node = doc.node(id).unwrap();
    let indent = "  ".repeat(depth);

    match &node.kind {
        NodeKind::Document => {}
        NodeKind::Element(name) => {
            output.push_str(&format!("| {indent}<{name}>\n"));
            // Sort attributes alphabetically for consistent comparison
            let mut attrs: Vec<(&String, &String)> = node.attributes.iter().collect();
            attrs.sort_by_key(|(k, _)| *k);
            for (key, val) in attrs {
                output.push_str(&format!("| {indent}  {key}=\"{val}\"\n"));
            }
            for &child_id in &node.children {
                serialize_node(doc, child_id, depth + 1, output);
            }
        }
        NodeKind::Text(text) => {
            output.push_str(&format!("| {indent}\"{text}\"\n"));
        }
        NodeKind::Comment(text) => {
            output.push_str(&format!("| {indent}<!-- {text} -->\n"));
        }
    }
}

fn run_test_file(filename: &str) -> (usize, usize, Vec<String>) {
    let path = fixtures_dir().join(filename);
    let tests = parse_test_file(&path);
    let mut total = 0;
    let mut passed = 0;
    let mut failures = Vec::new();

    for test in &tests {
        // Skip scripting-on tests (we don't support scripting)
        if test.script_on {
            continue;
        }
        // Skip fragment tests (not implemented)
        if test.is_fragment {
            continue;
        }

        total += 1;

        let result = parse(&test.input);
        let actual = serialize_tree_with_doctype(&result);

        if actual == test.expected_tree {
            passed += 1;
        } else {
            let fail_count = failures.len();
            if fail_count < 3 {
                failures.push(format!(
                    "FAIL: {}\n  input: {:?}\n  expected:\n{}\n  actual:\n{}",
                    test.description,
                    if test.input.len() > 80 {
                        format!("{}...", &test.input[..80])
                    } else {
                        test.input.clone()
                    },
                    test.expected_tree,
                    actual,
                ));
            } else {
                failures.push(format!("FAIL: {}", test.description));
            }
        }
    }

    (total, passed, failures)
}

#[test]
fn html5lib_tree_conformance() {
    let test_files: Vec<String> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .filter_map(|e| {
            let name = e.ok()?.file_name().to_string_lossy().to_string();
            if name.ends_with(".dat") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    let mut grand_total = 0;
    let mut grand_passed = 0;
    let mut grand_failures = 0;

    let mut file_results: Vec<(String, usize, usize)> = Vec::new();

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
        }
        file_results.push((filename.clone(), passed, total));
        grand_total += total;
        grand_passed += passed;
        grand_failures += fail_count;
    }

    // Sort by name for readable output
    file_results.sort_by(|a, b| a.0.cmp(&b.0));
    eprintln!("\n=== Per-file results ===");
    for (name, passed, total) in &file_results {
        let pct = if *total > 0 {
            (*passed as f64 / *total as f64) * 100.0
        } else {
            100.0
        };
        let status = if passed == total { " " } else { "!" };
        eprintln!("{status} {name}: {passed}/{total} ({pct:.0}%)");
    }

    let pass_rate = (grand_passed as f64 / grand_total as f64) * 100.0;
    eprintln!("\n=== TOTAL: {grand_passed}/{grand_total} passed ({pass_rate:.1}%) ===");

    // Target: 40% for initial implementation. The remaining failures are
    // from table edge cases, adoption agency details, foster parenting
    // precision, and missing features (foreign content, ruby, etc).
    // This will improve as we refine the tree builder.
    assert!(
        pass_rate >= 40.0,
        "html5lib tree conformance {pass_rate:.1}% < 40% ({grand_failures} failures out of {grand_total})",
    );
}
