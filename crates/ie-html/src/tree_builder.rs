use ie_dom::{Document, NodeId, NodeKind};

use crate::formatting::FormattingEntry;
use crate::insertion_mode::InsertionMode;
use crate::token::{Attribute, Token};
use crate::tokenizer::{Tokenizer, TokenizerState};

/// Result of parsing an HTML document.
pub struct ParseResult {
    pub document: Document,
    pub errors: Vec<String>,
    pub style_elements: Vec<String>,
    pub link_stylesheets: Vec<String>,
}

/// Top-level parse function. HTML parsing never fails — errors are collected.
pub fn parse(html: &str) -> ParseResult {
    let mut tb = TreeBuilder::new(html);
    tb.run();
    ParseResult {
        document: tb.doc,
        errors: tb.errors,
        style_elements: tb.style_elements,
        link_stylesheets: tb.link_stylesheets,
    }
}

struct TreeBuilder<'a> {
    doc: Document,
    tokenizer: Tokenizer<'a>,
    mode: InsertionMode,
    original_mode: Option<InsertionMode>,
    open_elements: Vec<NodeId>,
    active_formatting: Vec<FormattingEntry>,
    head_pointer: Option<NodeId>,
    form_pointer: Option<NodeId>,
    frameset_ok: bool,
    errors: Vec<String>,
    style_elements: Vec<String>,
    link_stylesheets: Vec<String>,
    pending_text: String,
    done: bool,
}

impl<'a> TreeBuilder<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            doc: Document::new(),
            tokenizer: Tokenizer::new(input),
            mode: InsertionMode::Initial,
            original_mode: None,
            open_elements: Vec::new(),
            active_formatting: Vec::new(),
            head_pointer: None,
            form_pointer: None,
            frameset_ok: true,
            errors: Vec::new(),
            style_elements: Vec::new(),
            link_stylesheets: Vec::new(),
            pending_text: String::new(),
            done: false,
        }
    }

    fn run(&mut self) {
        while !self.done {
            let token = match self.tokenizer.next() {
                Some(t) => t,
                None => {
                    self.done = true;
                    break;
                }
            };
            self.process_token(token);
        }
    }

    fn process_token(&mut self, token: Token) {
        match self.mode {
            InsertionMode::Initial => self.handle_initial(token),
            InsertionMode::BeforeHtml => self.handle_before_html(token),
            InsertionMode::BeforeHead => self.handle_before_head(token),
            InsertionMode::InHead => self.handle_in_head(token),
            InsertionMode::InHeadNoscript => self.handle_in_head_noscript(token),
            InsertionMode::AfterHead => self.handle_after_head(token),
            InsertionMode::InBody => self.handle_in_body(token),
            InsertionMode::Text => self.handle_text(token),
            InsertionMode::AfterBody => self.handle_after_body(token),
            InsertionMode::AfterAfterBody => self.handle_after_after_body(token),
            InsertionMode::InFrameset => self.handle_in_frameset(token),
            _ => {
                // Stub: unimplemented modes fall back to InBody
                self.handle_in_body(token);
            }
        }
    }

    // --- Helpers ---

    fn current_node(&self) -> Option<NodeId> {
        self.open_elements.last().copied()
    }

    fn current_node_name(&self) -> Option<&str> {
        let id = self.current_node()?;
        self.doc.node(id).and_then(|n| n.element_name())
    }

    fn element_name(&self, id: NodeId) -> Option<&str> {
        self.doc.node(id).and_then(|n| n.element_name())
    }

    fn insert_element(&mut self, name: &str, attrs: &[(String, String)]) -> NodeId {
        let id = self.doc.create_element(name);
        for (k, v) in attrs {
            self.doc.set_attribute(id, k, v);
        }
        let parent = self.current_node().unwrap_or(self.doc.root);
        let _ = self.doc.append_child(parent, id);
        self.open_elements.push(id);
        id
    }

    fn insert_element_at_root(&mut self, name: &str, attrs: &[(String, String)]) -> NodeId {
        let id = self.doc.create_element(name);
        for (k, v) in attrs {
            self.doc.set_attribute(id, k, v);
        }
        let _ = self.doc.append_child(self.doc.root, id);
        self.open_elements.push(id);
        id
    }

    fn insert_character(&mut self, c: char) {
        let parent = self.current_node().unwrap_or(self.doc.root);
        let children = self.doc.children(parent);
        if let Some(&last_child) = children.last()
            && let Some(node) = self.doc.node_mut(last_child)
            && let NodeKind::Text(ref mut text) = node.kind
        {
            text.push(c);
            return;
        }
        let id = self.doc.create_text(&c.to_string());
        let _ = self.doc.append_child(parent, id);
    }

    fn insert_comment(&mut self, data: &str) {
        let parent = self.current_node().unwrap_or(self.doc.root);
        let id = self.doc.create_comment(data);
        let _ = self.doc.append_child(parent, id);
    }

    fn insert_comment_at(&mut self, data: &str, parent: NodeId) {
        let id = self.doc.create_comment(data);
        let _ = self.doc.append_child(parent, id);
    }

    fn parse_error(&mut self, msg: &str) {
        tracing::warn!("parse error: {}", msg);
        self.errors.push(msg.to_string());
    }

    fn attrs_from_token(attributes: &[Attribute]) -> Vec<(String, String)> {
        attributes
            .iter()
            .map(|a| (a.name.clone(), a.value.clone()))
            .collect()
    }

    fn generate_implied_end_tags(&mut self, except: Option<&str>) {
        const IMPLIED: &[&str] = &[
            "dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt", "rtc",
        ];
        loop {
            match self.current_node_name() {
                Some(name) if IMPLIED.contains(&name) && except != Some(name) => {
                    // Need to compare without borrow conflict
                    self.open_elements.pop();
                }
                _ => break,
            }
        }
    }

    /// Scope boundaries for "in scope" checks.
    const SCOPE_BOUNDARIES: &'static [&'static str] = &[
        "applet",
        "caption",
        "html",
        "table",
        "td",
        "th",
        "marquee",
        "object",
        "template",
        "mi",
        "mo",
        "mn",
        "ms",
        "mtext",
        "annotation-xml",
        "foreignObject",
        "desc",
        "title",
    ];

    fn has_element_in_scope(&self, target: &str) -> bool {
        for &id in self.open_elements.iter().rev() {
            if let Some(name) = self.element_name(id) {
                if name == target {
                    return true;
                }
                if Self::SCOPE_BOUNDARIES.contains(&name) {
                    return false;
                }
            }
        }
        false
    }

    fn has_element_in_button_scope(&self, target: &str) -> bool {
        for &id in self.open_elements.iter().rev() {
            if let Some(name) = self.element_name(id) {
                if name == target {
                    return true;
                }
                if Self::SCOPE_BOUNDARIES.contains(&name) || name == "button" {
                    return false;
                }
            }
        }
        false
    }

    fn has_element_in_scope_by_set(&self, targets: &[&str]) -> bool {
        for &id in self.open_elements.iter().rev() {
            if let Some(name) = self.element_name(id) {
                if targets.contains(&name) {
                    return true;
                }
                if Self::SCOPE_BOUNDARIES.contains(&name) {
                    return false;
                }
            }
        }
        false
    }

    fn close_p_element(&mut self) {
        self.generate_implied_end_tags(Some("p"));
        while let Some(id) = self.open_elements.pop() {
            if self.element_name(id) == Some("p") {
                break;
            }
        }
    }

    fn pop_until(&mut self, target: &str) {
        while let Some(id) = self.open_elements.pop() {
            if self.element_name(id) == Some(target) {
                break;
            }
        }
    }

    fn pop_until_any(&mut self, targets: &[&str]) {
        while let Some(id) = self.open_elements.pop() {
            if let Some(name) = self.element_name(id)
                && targets.contains(&name)
            {
                break;
            }
        }
    }

    fn merge_attributes_into(&mut self, target_id: NodeId, attrs: &[Attribute]) {
        for attr in attrs {
            if self.doc.get_attribute(target_id, &attr.name).is_none() {
                self.doc.set_attribute(target_id, &attr.name, &attr.value);
            }
        }
    }

    // --- Generic parsing algorithms ---

    fn generic_rcdata_parsing(&mut self, name: &str, attrs: &[(String, String)]) {
        self.insert_element(name, attrs);
        self.tokenizer.set_state(TokenizerState::RcData);
        self.tokenizer.set_last_start_tag(name);
        self.original_mode = Some(self.mode);
        self.mode = InsertionMode::Text;
    }

    fn generic_raw_text_parsing(&mut self, name: &str, attrs: &[(String, String)]) {
        self.insert_element(name, attrs);
        self.tokenizer.set_state(TokenizerState::RawText);
        self.tokenizer.set_last_start_tag(name);
        self.original_mode = Some(self.mode);
        self.mode = InsertionMode::Text;
    }

    // --- Active formatting elements ---

    /// Push a formatting element entry (Noah's Ark clause: max 3 identical before last marker)
    fn push_active_formatting(
        &mut self,
        node_id: NodeId,
        tag_name: &str,
        attributes: &[Attribute],
    ) {
        let mut count = 0;
        let mut earliest_idx = None;
        for (i, entry) in self.active_formatting.iter().enumerate().rev() {
            match entry {
                FormattingEntry::Marker => break,
                FormattingEntry::Element {
                    tag_name: t,
                    attributes: a,
                    ..
                } => {
                    if t == tag_name && a == attributes {
                        count += 1;
                        earliest_idx = Some(i);
                    }
                }
            }
        }
        if count >= 3
            && let Some(idx) = earliest_idx
        {
            self.active_formatting.remove(idx);
        }
        self.active_formatting.push(FormattingEntry::Element {
            node_id,
            tag_name: tag_name.to_string(),
            attributes: attributes.to_vec(),
        });
    }

    /// Reconstruct active formatting elements (WHATWG 13.2.4.3)
    fn reconstruct_active_formatting(&mut self) {
        if self.active_formatting.is_empty() {
            return;
        }
        if let Some(last) = self.active_formatting.last() {
            if last.is_marker() {
                return;
            }
            if let Some(node_id) = last.node_id()
                && self.open_elements.contains(&node_id)
            {
                return;
            }
        }

        let mut idx = self.active_formatting.len() - 1;
        loop {
            if idx == 0 {
                break;
            }
            idx -= 1;
            let entry = &self.active_formatting[idx];
            if entry.is_marker() {
                idx += 1;
                break;
            }
            if let Some(node_id) = entry.node_id()
                && self.open_elements.contains(&node_id)
            {
                idx += 1;
                break;
            }
        }

        while idx < self.active_formatting.len() {
            let (tag_name, attributes) = match &self.active_formatting[idx] {
                FormattingEntry::Element {
                    tag_name,
                    attributes,
                    ..
                } => (tag_name.clone(), attributes.clone()),
                FormattingEntry::Marker => {
                    idx += 1;
                    continue;
                }
            };
            let attrs: Vec<(String, String)> = attributes
                .iter()
                .map(|a| (a.name.clone(), a.value.clone()))
                .collect();
            let new_id = self.insert_element(&tag_name, &attrs);
            self.active_formatting[idx] = FormattingEntry::Element {
                node_id: new_id,
                tag_name,
                attributes,
            };
            idx += 1;
        }
    }

    /// Clear active formatting elements to the last marker
    #[allow(dead_code)]
    fn clear_active_formatting_to_marker(&mut self) {
        while let Some(entry) = self.active_formatting.pop() {
            if entry.is_marker() {
                break;
            }
        }
    }

    /// Remove a node from active formatting by node_id
    #[allow(dead_code)]
    fn remove_from_active_formatting(&mut self, node_id: NodeId) {
        self.active_formatting
            .retain(|e| e.node_id() != Some(node_id));
    }

    /// The adoption agency algorithm (WHATWG 13.2.6.4.7)
    fn run_adoption_agency(&mut self, tag_name: &str) {
        // Step 1: if current node matches and is NOT in active_formatting, just pop
        if let Some(&current_id) = self.open_elements.last()
            && self.element_name(current_id) == Some(tag_name)
        {
            let in_formatting = self
                .active_formatting
                .iter()
                .any(|e| e.node_id() == Some(current_id));
            if !in_formatting {
                self.open_elements.pop();
                return;
            }
        }

        // Outer loop: 8 iterations max
        for _ in 0..8 {
            // Find formatting element — last in active_formatting with matching tag
            let formatting_idx = self.active_formatting.iter().rposition(
                |e| matches!(e, FormattingEntry::Element { tag_name: t, .. } if t == tag_name),
            );
            let Some(formatting_idx) = formatting_idx else {
                self.handle_any_other_end_tag(tag_name);
                return;
            };

            let formatting_node_id = match &self.active_formatting[formatting_idx] {
                FormattingEntry::Element { node_id, .. } => *node_id,
                _ => return,
            };

            // If formatting element not in open_elements
            let stack_idx = self
                .open_elements
                .iter()
                .rposition(|&id| id == formatting_node_id);
            let Some(stack_idx) = stack_idx else {
                self.parse_error("formatting element not in open elements");
                self.active_formatting.remove(formatting_idx);
                return;
            };

            // If formatting element not in scope
            if !self.has_element_in_scope(tag_name) {
                self.parse_error("formatting element not in scope");
                return;
            }

            // If formatting element is not the current node
            if self.open_elements.last() != Some(&formatting_node_id) {
                self.parse_error("formatting element is not current node");
            }

            // Find furthest block — first special element after formatting element in stack
            let furthest_block_idx = self.open_elements[stack_idx + 1..]
                .iter()
                .position(|&id| self.element_name(id).is_some_and(is_special_element))
                .map(|i| i + stack_idx + 1);

            let Some(furthest_block_idx) = furthest_block_idx else {
                // No furthest block: pop up to and including formatting element
                while self.open_elements.len() > stack_idx {
                    self.open_elements.pop();
                }
                self.active_formatting.remove(formatting_idx);
                return;
            };

            let furthest_block_id = self.open_elements[furthest_block_idx];
            let common_ancestor_id = self.open_elements[stack_idx.saturating_sub(1)];

            let mut bookmark = formatting_idx;
            let mut node_idx = furthest_block_idx;
            let mut last_node_id = furthest_block_id;

            // Inner loop
            for inner_count in 1..=3 {
                node_idx -= 1;
                let node_id = self.open_elements[node_idx];

                if node_id == formatting_node_id {
                    break;
                }

                let af_idx = self
                    .active_formatting
                    .iter()
                    .position(|e| e.node_id() == Some(node_id));

                if inner_count > 3
                    && let Some(af_idx) = af_idx
                {
                    self.active_formatting.remove(af_idx);
                    if af_idx < bookmark {
                        bookmark -= 1;
                    }
                    self.open_elements.remove(node_idx);
                    continue;
                }

                // If node is NOT in active_formatting, remove from open_elements
                let Some(af_idx) = af_idx else {
                    self.open_elements.remove(node_idx);
                    continue;
                };

                // Create replacement element
                let (tag, attrs) = match &self.active_formatting[af_idx] {
                    FormattingEntry::Element {
                        tag_name,
                        attributes,
                        ..
                    } => (tag_name.clone(), attributes.clone()),
                    _ => continue,
                };
                let attr_pairs: Vec<(String, String)> = attrs
                    .iter()
                    .map(|a| (a.name.clone(), a.value.clone()))
                    .collect();
                let new_id = self.doc.create_element(&tag);
                for (name, value) in &attr_pairs {
                    self.doc.set_attribute(new_id, name, value);
                }
                // Reparent node's children to new_id
                if let Some(node) = self.doc.node(node_id) {
                    let children: Vec<NodeId> = node.children.clone();
                    for &child in &children {
                        let _ = self.doc.remove_child(node_id, child);
                        let _ = self.doc.append_child(new_id, child);
                    }
                }
                // Replace node in its parent
                if let Some(parent_id) = self.doc.parent(node_id) {
                    let _ = self.doc.remove_child(parent_id, node_id);
                    let _ = self.doc.append_child(parent_id, new_id);
                }

                self.active_formatting[af_idx] = FormattingEntry::Element {
                    node_id: new_id,
                    tag_name: tag,
                    attributes: attrs,
                };
                self.open_elements[node_idx] = new_id;

                if last_node_id == furthest_block_id {
                    bookmark = af_idx + 1;
                }

                // Reparent last_node to new_id
                if let Some(parent_id) = self.doc.parent(last_node_id) {
                    let _ = self.doc.remove_child(parent_id, last_node_id);
                }
                let _ = self.doc.append_child(new_id, last_node_id);
                last_node_id = new_id;
            }

            // Insert last_node into common ancestor
            if let Some(parent_id) = self.doc.parent(last_node_id) {
                let _ = self.doc.remove_child(parent_id, last_node_id);
            }
            let _ = self.doc.append_child(common_ancestor_id, last_node_id);

            // Create new element for the formatting element
            let fmt_idx = formatting_idx.min(self.active_formatting.len().saturating_sub(1));
            let (tag, attrs) = match &self.active_formatting[fmt_idx] {
                FormattingEntry::Element {
                    tag_name,
                    attributes,
                    ..
                } => (tag_name.clone(), attributes.clone()),
                _ => return,
            };
            let new_formatting_id = self.doc.create_element(&tag);
            for a in &attrs {
                self.doc.set_attribute(new_formatting_id, &a.name, &a.value);
            }

            // Take all children of furthest block and append to new formatting element
            if let Some(fb_node) = self.doc.node(furthest_block_id) {
                let children: Vec<NodeId> = fb_node.children.clone();
                for &child in &children {
                    let _ = self.doc.remove_child(furthest_block_id, child);
                    let _ = self.doc.append_child(new_formatting_id, child);
                }
            }

            // Append new formatting element to furthest block
            let _ = self.doc.append_child(furthest_block_id, new_formatting_id);

            // Remove old formatting entry, insert new one at bookmark
            let old_fmt_idx = self
                .active_formatting
                .iter()
                .position(|e| e.node_id() == Some(formatting_node_id));
            if let Some(old_idx) = old_fmt_idx {
                self.active_formatting.remove(old_idx);
                if old_idx < bookmark {
                    bookmark -= 1;
                }
            }
            let new_entry = FormattingEntry::Element {
                node_id: new_formatting_id,
                tag_name: tag,
                attributes: attrs,
            };
            let insert_pos = bookmark.min(self.active_formatting.len());
            self.active_formatting.insert(insert_pos, new_entry);

            // Update open_elements
            self.open_elements.retain(|&id| id != formatting_node_id);
            if let Some(fb_pos) = self
                .open_elements
                .iter()
                .position(|&id| id == furthest_block_id)
            {
                self.open_elements.insert(fb_pos + 1, new_formatting_id);
            }
        }
    }

    // --- Insertion mode handlers ---

    fn handle_initial(&mut self, token: Token) {
        match token {
            Token::Character(c) if is_whitespace(c) => {
                // ignore
            }
            Token::Comment(ref data) => {
                self.insert_comment_at(data, self.doc.root);
            }
            Token::Doctype { .. } => {
                // Accept doctype, always no-quirks mode
            }
            _ => {
                self.parse_error("unexpected token in Initial mode");
                self.mode = InsertionMode::BeforeHtml;
                self.process_token(token);
            }
        }
    }

    fn handle_before_html(&mut self, token: Token) {
        match token {
            Token::Doctype { .. } => {
                self.parse_error("doctype in BeforeHtml");
            }
            Token::Comment(ref data) => {
                self.insert_comment_at(data, self.doc.root);
            }
            Token::Character(c) if is_whitespace(c) => {
                // ignore
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "html" => {
                let attrs = Self::attrs_from_token(attributes);
                self.insert_element_at_root("html", &attrs);
                self.mode = InsertionMode::BeforeHead;
            }
            Token::EndTag { ref name }
                if name == "head" || name == "body" || name == "html" || name == "br" =>
            {
                // Act as if start tag "html"
                self.insert_element_at_root("html", &[]);
                self.mode = InsertionMode::BeforeHead;
                self.process_token(token);
            }
            Token::EndTag { .. } => {
                self.parse_error("unexpected end tag in BeforeHtml");
            }
            Token::Eof => {
                self.insert_element_at_root("html", &[]);
                self.mode = InsertionMode::BeforeHead;
                self.done = true;
            }
            _ => {
                self.insert_element_at_root("html", &[]);
                self.mode = InsertionMode::BeforeHead;
                self.process_token(token);
            }
        }
    }

    fn handle_before_head(&mut self, token: Token) {
        match token {
            Token::Character(c) if is_whitespace(c) => {
                // ignore
            }
            Token::Comment(ref data) => {
                self.insert_comment(data);
            }
            Token::Doctype { .. } => {
                self.parse_error("doctype in BeforeHead");
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "html" => {
                self.handle_in_body_start_html(attributes);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "head" => {
                let attrs = Self::attrs_from_token(attributes);
                let id = self.insert_element("head", &attrs);
                self.head_pointer = Some(id);
                self.mode = InsertionMode::InHead;
            }
            Token::EndTag { ref name }
                if name == "head" || name == "body" || name == "html" || name == "br" =>
            {
                // Implicit head
                let id = self.insert_element("head", &[]);
                self.head_pointer = Some(id);
                self.mode = InsertionMode::InHead;
                self.process_token(token);
            }
            Token::EndTag { .. } => {
                self.parse_error("unexpected end tag in BeforeHead");
            }
            Token::Eof => {
                let id = self.insert_element("head", &[]);
                self.head_pointer = Some(id);
                self.mode = InsertionMode::InHead;
                self.done = true;
            }
            _ => {
                let id = self.insert_element("head", &[]);
                self.head_pointer = Some(id);
                self.mode = InsertionMode::InHead;
                self.process_token(token);
            }
        }
    }

    fn handle_in_head(&mut self, token: Token) {
        match token {
            Token::Character(c) if is_whitespace(c) => {
                self.insert_character(c);
            }
            Token::Comment(ref data) => {
                self.insert_comment(data);
            }
            Token::Doctype { .. } => {
                self.parse_error("doctype in InHead");
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "html" => {
                self.handle_in_body_start_html(attributes);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "base" || name == "basefont" || name == "bgsound" || name == "link" => {
                let attrs = Self::attrs_from_token(attributes);
                // Capture link stylesheets
                if name == "link" {
                    let rel = attrs
                        .iter()
                        .find(|(k, _)| k == "rel")
                        .map(|(_, v)| v.as_str());
                    let href = attrs
                        .iter()
                        .find(|(k, _)| k == "href")
                        .map(|(_, v)| v.clone());
                    if rel == Some("stylesheet")
                        && let Some(href) = href
                    {
                        self.link_stylesheets.push(href);
                    }
                }
                self.insert_element(name, &attrs);
                // Void element: pop immediately
                self.open_elements.pop();
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "meta" => {
                let attrs = Self::attrs_from_token(attributes);
                self.insert_element("meta", &attrs);
                self.open_elements.pop();
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "title" => {
                let attrs = Self::attrs_from_token(attributes);
                self.generic_rcdata_parsing("title", &attrs);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "noframes" || name == "style" => {
                let attrs = Self::attrs_from_token(attributes);
                let tag = name.clone();
                self.generic_raw_text_parsing(&tag, &attrs);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "noscript" => {
                let attrs = Self::attrs_from_token(attributes);
                self.insert_element("noscript", &attrs);
                self.mode = InsertionMode::InHeadNoscript;
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "script" => {
                let attrs = Self::attrs_from_token(attributes);
                self.insert_element("script", &attrs);
                self.tokenizer.set_state(TokenizerState::ScriptData);
                self.tokenizer.set_last_start_tag("script");
                self.original_mode = Some(self.mode);
                self.mode = InsertionMode::Text;
            }
            Token::EndTag { ref name } if name == "head" => {
                self.open_elements.pop();
                self.mode = InsertionMode::AfterHead;
            }
            Token::EndTag { ref name } if name == "body" || name == "html" || name == "br" => {
                // Act as EndTag "head"
                self.open_elements.pop();
                self.mode = InsertionMode::AfterHead;
                self.process_token(token);
            }
            Token::StartTag { ref name, .. } if name == "head" => {
                self.parse_error("duplicate head tag");
            }
            Token::EndTag { .. } => {
                self.parse_error("unexpected end tag in InHead");
            }
            Token::Eof => {
                self.open_elements.pop();
                self.mode = InsertionMode::AfterHead;
                self.done = true;
            }
            _ => {
                // Act as EndTag "head", reprocess
                self.open_elements.pop();
                self.mode = InsertionMode::AfterHead;
                self.process_token(token);
            }
        }
    }

    fn handle_in_head_noscript(&mut self, token: Token) {
        match token {
            Token::Doctype { .. } => {
                self.parse_error("doctype in InHeadNoscript");
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "html" => {
                self.handle_in_body_start_html(attributes);
            }
            Token::EndTag { ref name } if name == "noscript" => {
                self.open_elements.pop();
                self.mode = InsertionMode::InHead;
            }
            Token::Character(c) if is_whitespace(c) => {
                self.handle_in_head(token);
            }
            Token::Comment(_) => {
                self.handle_in_head(token);
            }
            Token::StartTag { ref name, .. }
                if name == "basefont"
                    || name == "bgsound"
                    || name == "link"
                    || name == "meta"
                    || name == "noframes"
                    || name == "style" =>
            {
                self.handle_in_head(token);
            }
            Token::StartTag { ref name, .. } if name == "head" || name == "noscript" => {
                self.parse_error("unexpected start tag in InHeadNoscript");
            }
            Token::EndTag { ref name } if name != "br" => {
                self.parse_error("unexpected end tag in InHeadNoscript");
            }
            _ => {
                self.parse_error("unexpected token in InHeadNoscript");
                self.open_elements.pop();
                self.mode = InsertionMode::InHead;
                self.process_token(token);
            }
        }
    }

    fn handle_after_head(&mut self, token: Token) {
        match token {
            Token::Character(c) if is_whitespace(c) => {
                self.insert_character(c);
            }
            Token::Comment(ref data) => {
                self.insert_comment(data);
            }
            Token::Doctype { .. } => {
                self.parse_error("doctype in AfterHead");
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "html" => {
                self.handle_in_body_start_html(attributes);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "body" => {
                let attrs = Self::attrs_from_token(attributes);
                self.insert_element("body", &attrs);
                self.frameset_ok = false;
                self.mode = InsertionMode::InBody;
            }
            Token::StartTag { ref name, .. } if name == "frameset" => {
                let attrs = Self::attrs_from_token(match &token {
                    Token::StartTag { attributes, .. } => attributes,
                    _ => unreachable!(),
                });
                self.insert_element("frameset", &attrs);
                self.mode = InsertionMode::InFrameset;
            }
            Token::StartTag { ref name, .. }
                if matches!(
                    name.as_str(),
                    "base"
                        | "basefont"
                        | "bgsound"
                        | "link"
                        | "meta"
                        | "noframes"
                        | "script"
                        | "style"
                        | "template"
                        | "title"
                ) =>
            {
                self.parse_error("head element after head");
                // Push head back, process as InHead, remove head
                if let Some(head) = self.head_pointer {
                    self.open_elements.push(head);
                    self.handle_in_head(token);
                    self.open_elements.retain(|&id| id != head);
                }
            }
            Token::EndTag { ref name } if name == "body" || name == "html" || name == "br" => {
                // Implicit body
                self.insert_element("body", &[]);
                self.mode = InsertionMode::InBody;
                self.process_token(token);
            }
            Token::StartTag { ref name, .. } if name == "head" => {
                self.parse_error("head start tag in AfterHead");
            }
            Token::EndTag { .. } => {
                self.parse_error("unexpected end tag in AfterHead");
            }
            Token::Eof => {
                self.insert_element("body", &[]);
                self.mode = InsertionMode::InBody;
                self.done = true;
            }
            _ => {
                // Implicit body
                self.insert_element("body", &[]);
                self.mode = InsertionMode::InBody;
                self.process_token(token);
            }
        }
    }

    fn handle_in_body(&mut self, token: Token) {
        match token {
            Token::Character('\0') => {
                self.parse_error("null character in body");
            }
            Token::Character(c) if is_whitespace(c) => {
                self.insert_character(c);
            }
            Token::Character(c) => {
                self.reconstruct_active_formatting();
                self.insert_character(c);
                self.frameset_ok = false;
            }
            Token::Comment(ref data) => {
                self.insert_comment(data);
            }
            Token::Doctype { .. } => {
                self.parse_error("doctype in InBody");
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "html" => {
                self.handle_in_body_start_html(attributes);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "body" => {
                self.parse_error("body start tag in InBody");
                // Merge attrs into existing body if it's second on stack
                if self.open_elements.len() >= 2 {
                    let body_id = self.open_elements[1];
                    if self.element_name(body_id) == Some("body") {
                        self.merge_attributes_into(body_id, attributes);
                    }
                }
            }
            Token::StartTag { ref name, .. }
                if matches!(
                    name.as_str(),
                    "base"
                        | "basefont"
                        | "bgsound"
                        | "link"
                        | "meta"
                        | "noframes"
                        | "script"
                        | "style"
                        | "template"
                        | "title"
                ) =>
            {
                self.handle_in_head(token);
            }
            Token::EndTag { ref name } if name == "template" => {
                self.handle_in_head(token);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if is_block_element(name) => {
                if self.has_element_in_button_scope("p") {
                    self.close_p_element();
                }
                let attrs = Self::attrs_from_token(attributes);
                let tag = name.clone();
                self.insert_element(&tag, &attrs);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if is_heading(name) => {
                if self.has_element_in_button_scope("p") {
                    self.close_p_element();
                }
                if self.current_node_name().is_some_and(is_heading) {
                    self.parse_error("heading inside heading");
                    self.open_elements.pop();
                }
                let attrs = Self::attrs_from_token(attributes);
                let tag = name.clone();
                self.insert_element(&tag, &attrs);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "pre" || name == "listing" => {
                if self.has_element_in_button_scope("p") {
                    self.close_p_element();
                }
                let attrs = Self::attrs_from_token(attributes);
                let tag = name.clone();
                self.insert_element(&tag, &attrs);
                self.frameset_ok = false;
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "form" => {
                if self.form_pointer.is_some() {
                    self.parse_error("nested form element");
                    return;
                }
                if self.has_element_in_button_scope("p") {
                    self.close_p_element();
                }
                let attrs = Self::attrs_from_token(attributes);
                let id = self.insert_element("form", &attrs);
                self.form_pointer = Some(id);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if is_void_element(name) => {
                if name == "hr" && self.has_element_in_button_scope("p") {
                    self.close_p_element();
                }
                let attrs = Self::attrs_from_token(attributes);
                let tag = name.clone();
                self.insert_element(&tag, &attrs);
                self.open_elements.pop();
                if name != "input" {
                    self.frameset_ok = false;
                }
            }
            Token::EndTag { ref name } if name == "body" => {
                if !self.has_element_in_scope("body") {
                    self.parse_error("body end tag without body in scope");
                    return;
                }
                self.mode = InsertionMode::AfterBody;
            }
            Token::EndTag { ref name } if name == "html" => {
                if !self.has_element_in_scope("body") {
                    self.parse_error("html end tag without body in scope");
                    return;
                }
                self.mode = InsertionMode::AfterBody;
                self.process_token(token);
            }
            Token::EndTag { ref name } if name == "p" => {
                if !self.has_element_in_button_scope("p") {
                    self.parse_error("p end tag without p in scope");
                    self.insert_element("p", &[]);
                }
                self.close_p_element();
            }
            Token::EndTag { ref name } if is_block_element(name) => {
                let tag = name.clone();
                if !self.has_element_in_scope(&tag) {
                    self.parse_error("end tag without matching start tag in scope");
                    return;
                }
                self.generate_implied_end_tags(None);
                self.pop_until(&tag);
            }
            Token::EndTag { ref name } if is_heading(name) => {
                let headings = ["h1", "h2", "h3", "h4", "h5", "h6"];
                if !self.has_element_in_scope_by_set(&headings) {
                    self.parse_error("heading end tag without heading in scope");
                    return;
                }
                self.generate_implied_end_tags(None);
                self.pop_until_any(&headings);
            }
            Token::EndTag { ref name } if name == "form" => {
                let node = self.form_pointer.take();
                if let Some(form_id) = node {
                    if !self.has_element_in_scope("form") {
                        return;
                    }
                    self.generate_implied_end_tags(None);
                    self.open_elements.retain(|&id| id != form_id);
                } else {
                    self.parse_error("form end tag without form pointer");
                }
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "a" => {
                // Special case: <a> inside <a> — run adoption agency first
                let has_active_a = self.active_formatting.iter().rev().any(|e| {
                    if e.is_marker() {
                        return false;
                    }
                    e.tag_name() == Some("a")
                });
                if has_active_a {
                    self.parse_error("nested a element");
                    self.run_adoption_agency("a");
                    // Remove from active_formatting and open_elements if still present
                    if let Some(pos) = self
                        .active_formatting
                        .iter()
                        .position(|e| e.tag_name() == Some("a"))
                    {
                        let old_id = self.active_formatting[pos].node_id();
                        self.active_formatting.remove(pos);
                        if let Some(old_id) = old_id {
                            self.open_elements.retain(|&id| id != old_id);
                        }
                    }
                }
                self.reconstruct_active_formatting();
                let attrs = Self::attrs_from_token(attributes);
                let token_attrs = attributes.clone();
                let id = self.insert_element("a", &attrs);
                self.push_active_formatting(id, "a", &token_attrs);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if is_formatting_element(name) => {
                self.reconstruct_active_formatting();
                let attrs = Self::attrs_from_token(attributes);
                let tag = name.clone();
                let token_attrs = attributes.clone();
                let id = self.insert_element(&tag, &attrs);
                self.push_active_formatting(id, &tag, &token_attrs);
            }
            Token::EndTag { ref name } if is_formatting_element(name) => {
                let tag = name.clone();
                self.run_adoption_agency(&tag);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } => {
                // Any other start tag
                self.reconstruct_active_formatting();
                let attrs = Self::attrs_from_token(attributes);
                let tag = name.clone();
                self.insert_element(&tag, &attrs);
            }
            Token::EndTag { ref name } => {
                // Any other end tag
                let tag = name.clone();
                self.handle_any_other_end_tag(&tag);
            }
            Token::Eof => {
                self.done = true;
            }
        }
    }

    fn handle_in_body_start_html(&mut self, attributes: &[Attribute]) {
        self.parse_error("html start tag in body");
        if let Some(&html_id) = self.open_elements.first() {
            self.merge_attributes_into(html_id, attributes);
        }
    }

    fn handle_any_other_end_tag(&mut self, tag: &str) {
        // Walk stack from top, looking for matching element
        for i in (0..self.open_elements.len()).rev() {
            let id = self.open_elements[i];
            if self.element_name(id) == Some(tag) {
                self.generate_implied_end_tags(Some(tag));
                self.open_elements.truncate(i);
                return;
            }
            // If we hit a special element, stop
            if self.element_name(id).is_some_and(is_special_element) {
                self.parse_error("end tag has no matching start tag");
                return;
            }
        }
    }

    fn handle_text(&mut self, token: Token) {
        match token {
            Token::Character(c) => {
                self.insert_character(c);
                self.pending_text.push(c);
            }
            Token::Eof => {
                self.parse_error("unexpected EOF in text mode");
                // Check if current is style before popping
                if self.current_node_name() == Some("style") {
                    self.style_elements.push(self.pending_text.clone());
                }
                self.pending_text.clear();
                self.open_elements.pop();
                self.mode = self.original_mode.take().unwrap_or(InsertionMode::InBody);
                self.process_token(token);
            }
            Token::EndTag { ref name } if name == "script" => {
                self.pending_text.clear();
                self.open_elements.pop();
                self.mode = self.original_mode.take().unwrap_or(InsertionMode::InBody);
            }
            Token::EndTag { .. } => {
                // Check if current element is "style" and capture text
                if self.current_node_name() == Some("style") {
                    self.style_elements.push(self.pending_text.clone());
                }
                self.pending_text.clear();
                self.open_elements.pop();
                self.mode = self.original_mode.take().unwrap_or(InsertionMode::InBody);
            }
            _ => {
                // Shouldn't happen in Text mode, but handle gracefully
            }
        }
    }

    fn handle_after_body(&mut self, token: Token) {
        match token {
            Token::Character(c) if is_whitespace(c) => {
                self.handle_in_body(token);
            }
            Token::Comment(ref data) => {
                // Insert into html element (first on stack)
                if let Some(&html_id) = self.open_elements.first() {
                    self.insert_comment_at(data, html_id);
                }
            }
            Token::Doctype { .. } => {
                self.parse_error("doctype in AfterBody");
            }
            Token::StartTag { ref name, .. } if name == "html" => {
                self.handle_in_body(token);
            }
            Token::EndTag { ref name } if name == "html" => {
                self.mode = InsertionMode::AfterAfterBody;
            }
            Token::Eof => {
                self.done = true;
            }
            _ => {
                self.parse_error("unexpected token in AfterBody");
                self.mode = InsertionMode::InBody;
                self.process_token(token);
            }
        }
    }

    fn handle_after_after_body(&mut self, token: Token) {
        match token {
            Token::Comment(ref data) => {
                self.insert_comment_at(data, self.doc.root);
            }
            Token::Doctype { .. }
            | Token::Character('\t')
            | Token::Character('\n')
            | Token::Character('\x0C')
            | Token::Character('\r')
            | Token::Character(' ') => {
                self.handle_in_body(token);
            }
            Token::StartTag { ref name, .. } if name == "html" => {
                self.handle_in_body(token);
            }
            Token::Eof => {
                self.done = true;
            }
            _ => {
                self.parse_error("unexpected token in AfterAfterBody");
                self.mode = InsertionMode::InBody;
                self.process_token(token);
            }
        }
    }

    fn handle_in_frameset(&mut self, token: Token) {
        // Stub — minimal handling
        match token {
            Token::Character(c) if is_whitespace(c) => {
                self.insert_character(c);
            }
            Token::Comment(ref data) => {
                self.insert_comment(data);
            }
            Token::EndTag { ref name } if name == "frameset" => {
                self.open_elements.pop();
                // If not the html element and not frameset, switch to AfterFrameset
                if self.current_node_name() != Some("html") {
                    self.mode = InsertionMode::AfterFrameset;
                }
            }
            Token::Eof => {
                self.done = true;
            }
            _ => {
                self.parse_error("unexpected token in InFrameset");
            }
        }
    }
}

fn is_whitespace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' ')
}

fn is_heading(name: &str) -> bool {
    matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

fn is_block_element(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "center"
            | "details"
            | "dialog"
            | "dir"
            | "div"
            | "dl"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "header"
            | "hgroup"
            | "main"
            | "menu"
            | "nav"
            | "ol"
            | "p"
            | "search"
            | "section"
            | "summary"
            | "ul"
    )
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area" | "br" | "embed" | "img" | "keygen" | "wbr" | "hr" | "input"
    )
}

fn is_formatting_element(name: &str) -> bool {
    matches!(
        name,
        "a" | "b"
            | "big"
            | "code"
            | "em"
            | "font"
            | "i"
            | "s"
            | "small"
            | "strike"
            | "strong"
            | "tt"
            | "u"
    )
}

fn is_special_element(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "applet"
            | "area"
            | "article"
            | "aside"
            | "base"
            | "basefont"
            | "bgsound"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "caption"
            | "center"
            | "col"
            | "colgroup"
            | "dd"
            | "details"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "embed"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "iframe"
            | "img"
            | "input"
            | "li"
            | "link"
            | "listing"
            | "main"
            | "marquee"
            | "menu"
            | "meta"
            | "nav"
            | "noembed"
            | "noframes"
            | "noscript"
            | "object"
            | "ol"
            | "p"
            | "param"
            | "pre"
            | "script"
            | "search"
            | "section"
            | "select"
            | "source"
            | "style"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "title"
            | "tr"
            | "track"
            | "ul"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ie_dom::NodeKind;

    fn parse_and_check(html: &str) -> ParseResult {
        parse(html)
    }

    /// Helper: get element names of direct children of a node
    fn child_names(result: &ParseResult, id: NodeId) -> Vec<String> {
        result
            .document
            .children(id)
            .iter()
            .filter_map(|&cid| match &result.document.node(cid)?.kind {
                NodeKind::Element(name) => Some(name.clone()),
                NodeKind::Text(t) => Some(format!("#text:{t}")),
                NodeKind::Comment(c) => Some(format!("#comment:{c}")),
                NodeKind::Document => Some("#document".to_string()),
            })
            .collect()
    }

    fn find_element<'a>(result: &'a ParseResult, tag: &str) -> Option<NodeId> {
        result
            .document
            .get_elements_by_tag_name(result.document.root, tag)
            .into_iter()
            .next()
    }

    #[test]
    fn minimal_document() {
        let result = parse_and_check("<!DOCTYPE html><html><head></head><body></body></html>");
        let root = result.document.root;
        let children = child_names(&result, root);
        assert_eq!(children, vec!["html"]);

        let html_id = find_element(&result, "html").unwrap();
        let html_children = child_names(&result, html_id);
        assert_eq!(html_children, vec!["head", "body"]);
    }

    #[test]
    fn implicit_elements() {
        let result = parse_and_check("Hello");
        let root = result.document.root;
        let children = child_names(&result, root);
        assert_eq!(children, vec!["html"]);

        let html_id = find_element(&result, "html").unwrap();
        let html_children = child_names(&result, html_id);
        assert_eq!(html_children, vec!["head", "body"]);

        let body_id = find_element(&result, "body").unwrap();
        let body_children = child_names(&result, body_id);
        assert_eq!(body_children, vec!["#text:Hello"]);
    }

    #[test]
    fn paragraph() {
        let result = parse_and_check("<p>Hello</p>");
        let body_id = find_element(&result, "body").unwrap();
        let body_children = child_names(&result, body_id);
        assert_eq!(body_children, vec!["p"]);

        let p_id = find_element(&result, "p").unwrap();
        let p_children = child_names(&result, p_id);
        assert_eq!(p_children, vec!["#text:Hello"]);
    }

    #[test]
    fn void_elements() {
        let result = parse_and_check("<br><img src=\"x\"><hr>");
        let body_id = find_element(&result, "body").unwrap();
        let body_children = child_names(&result, body_id);
        assert_eq!(body_children, vec!["br", "img", "hr"]);

        // Void elements have no children
        let br_id = find_element(&result, "br").unwrap();
        assert!(result.document.children(br_id).is_empty());
        let img_id = find_element(&result, "img").unwrap();
        assert!(result.document.children(img_id).is_empty());
        assert_eq!(result.document.get_attribute(img_id, "src"), Some("x"));
    }

    #[test]
    fn style_extraction() {
        let result = parse_and_check("<style>.a{}</style>");
        assert_eq!(result.style_elements, vec![".a{}"]);
    }

    #[test]
    fn link_extraction() {
        let result = parse_and_check("<link rel=\"stylesheet\" href=\"s.css\">");
        assert_eq!(result.link_stylesheets, vec!["s.css"]);
    }

    #[test]
    fn nested_divs() {
        let result = parse_and_check("<div><p>text</p></div>");
        let div_id = find_element(&result, "div").unwrap();
        let div_children = child_names(&result, div_id);
        assert_eq!(div_children, vec!["p"]);

        let p_id = find_element(&result, "p").unwrap();
        let p_children = child_names(&result, p_id);
        assert_eq!(p_children, vec!["#text:text"]);
    }

    #[test]
    fn heading_levels() {
        let result = parse_and_check("<h1>A</h1><h2>B</h2>");
        let body_id = find_element(&result, "body").unwrap();
        let body_children = child_names(&result, body_id);
        assert_eq!(body_children, vec!["h1", "h2"]);
    }

    #[test]
    fn implicit_body_close() {
        let result = parse_and_check("<body><p>A</body><p>B");
        let body_id = find_element(&result, "body").unwrap();
        let body_children = child_names(&result, body_id);
        // Both p elements should be in the body
        assert_eq!(body_children, vec!["p", "p"]);
    }

    #[test]
    fn comment_in_body() {
        // Comment before any tags gets inserted at document root (Initial mode)
        let result = parse_and_check("<!-- hello --><p>text</p>");
        let root = result.document.root;
        let root_children = child_names(&result, root);
        assert_eq!(root_children, vec!["#comment: hello ", "html"]);

        let body_id = find_element(&result, "body").unwrap();
        let body_children = child_names(&result, body_id);
        assert_eq!(body_children, vec!["p"]);

        // Comment inside body goes into body
        let result2 = parse_and_check("<p>text</p><!-- hello -->");
        let body_id2 = find_element(&result2, "body").unwrap();
        let body_children2 = child_names(&result2, body_id2);
        assert_eq!(body_children2, vec!["p", "#comment: hello "]);
    }

    #[test]
    fn doctype_handling() {
        let result = parse_and_check("<!DOCTYPE html><p>text");
        let body_id = find_element(&result, "body").unwrap();
        let body_children = child_names(&result, body_id);
        assert_eq!(body_children, vec!["p"]);
    }

    #[test]
    fn title_rcdata() {
        let result = parse_and_check("<title>a < b</title>");
        let title_id = find_element(&result, "title").unwrap();
        let title_children = child_names(&result, title_id);
        assert_eq!(title_children, vec!["#text:a < b"]);
    }

    fn first_child_element(doc: &Document, parent: NodeId, name: &str) -> Option<NodeId> {
        doc.node(parent)?
            .children
            .iter()
            .find(|&&id| doc.node(id).and_then(|n| n.element_name()) == Some(name))
            .copied()
    }

    fn assert_has_text_child(doc: &Document, parent: NodeId, expected: &str) {
        let node = doc.node(parent).unwrap();
        let has_text = node
            .children
            .iter()
            .any(|&id| matches!(doc.node(id), Some(n) if n.text_content() == Some(expected)));
        assert!(has_text, "expected text child '{expected}' in element");
    }

    #[test]
    fn properly_nested_formatting() {
        // <b><i>text</i></b> -> body > b > i > "text"
        let result = parse("<b><i>text</i></b>");
        let body = find_element(&result, "body").unwrap();
        let b = first_child_element(&result.document, body, "b").unwrap();
        let i = first_child_element(&result.document, b, "i").unwrap();
        assert_has_text_child(&result.document, i, "text");
    }

    #[test]
    fn misnested_formatting() {
        // <p><b>bold<i>both</b>italic</i></p>
        // The adoption agency should restructure the tree
        let result = parse("<p><b>bold<i>both</b>italic</i></p>");
        let body = find_element(&result, "body").unwrap();
        let p = first_child_element(&result.document, body, "p").unwrap();
        let p_node = result.document.node(p).unwrap();
        assert!(
            p_node.children.len() >= 2,
            "p should have restructured children, got: {:?}",
            child_names(&result, p)
        );
    }

    #[test]
    fn a_inside_a() {
        // <a href="1">first<a href="2">second</a>
        // First <a> should be closed by adoption agency before second <a>
        let result = parse("<a href=\"1\">first<a href=\"2\">second</a>");
        let body = find_element(&result, "body").unwrap();
        let body_node = result.document.node(body).unwrap();
        let a_elements: Vec<NodeId> = body_node
            .children
            .iter()
            .filter(|&&id| result.document.node(id).and_then(|n| n.element_name()) == Some("a"))
            .copied()
            .collect();
        assert_eq!(a_elements.len(), 2, "should have 2 <a> elements");
    }
}
