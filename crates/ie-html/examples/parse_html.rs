use ie_html::parse;

fn main() {
    let html = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "<p>Hello <b>bold <i>both</b> italic</i> world</p>".to_string());

    let result = parse(&html);

    println!("=== DOM Tree ===");
    print_tree(&result.document, result.document.root, 0);

    if !result.errors.is_empty() {
        println!("\n=== Parse Errors ({}) ===", result.errors.len());
        for e in &result.errors {
            println!("  - {e}");
        }
    }
    if !result.style_elements.is_empty() {
        println!("\n=== Styles ===");
        for s in &result.style_elements {
            println!("  {s}");
        }
    }
    if !result.link_stylesheets.is_empty() {
        println!("\n=== Linked Stylesheets ===");
        for l in &result.link_stylesheets {
            println!("  {l}");
        }
    }
}

fn print_tree(doc: &ie_dom::Document, id: ie_dom::NodeId, indent: usize) {
    let node = doc.node(id).unwrap();
    let prefix = "| ".repeat(indent);
    match &node.kind {
        ie_dom::NodeKind::Document => println!("{prefix}#document"),
        ie_dom::NodeKind::Doctype { name, .. } => println!("{prefix}<!DOCTYPE {name}>"),
        ie_dom::NodeKind::Element(name) => {
            let attrs: Vec<String> = node
                .attributes
                .iter()
                .map(|(k, v)| format!("{k}=\"{v}\""))
                .collect();
            if attrs.is_empty() {
                println!("{prefix}<{name}>");
            } else {
                println!("{prefix}<{name} {}>", attrs.join(" "));
            }
        }
        ie_dom::NodeKind::Text(text) => println!("{prefix}\"{text}\""),
        ie_dom::NodeKind::Comment(text) => println!("{prefix}<!-- {text} -->"),
    }
    for &child in &node.children {
        print_tree(doc, child, indent + 1);
    }
}
