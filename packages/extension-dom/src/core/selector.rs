use super::node::NodeData;
use super::Document;

// ── Attribute selector ────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum AttrOp {
    Exists,      // [attr]
    Equals,      // [attr="val"]
    Contains,    // [attr~="val"]  — word in whitespace-separated list
    DashPrefix,  // [attr|="val"]  — equals val or starts with val-
    StartsWith,  // [attr^="val"]
    EndsWith,    // [attr$="val"]
    Includes,    // [attr*="val"]  — substring
}

struct AttrSel<'a> {
    name: &'a str,
    op: AttrOp,
    value: &'a str,
}

fn parse_attr_sel(inner: &str) -> AttrSel<'_> {
    let inner = inner.trim();
    // Find the first operator character
    let op_pos = inner.find(['=', '~', '|', '^', '$', '*']);
    let Some(pos) = op_pos else {
        // [attr] — existence only
        return AttrSel { name: inner, op: AttrOp::Exists, value: "" };
    };

    let ch = inner.as_bytes()[pos];
    if ch == b'=' {
        // Simple equals: [attr=val] or [attr="val"]
        let name = inner[..pos].trim();
        let value = strip_quotes(inner[pos + 1..].trim());
        AttrSel { name, op: AttrOp::Equals, value }
    } else if pos + 1 < inner.len() && inner.as_bytes()[pos + 1] == b'=' {
        // Two-char operator: ~= |= ^= $= *=
        let name = inner[..pos].trim();
        let op = match ch {
            b'~' => AttrOp::Contains,
            b'|' => AttrOp::DashPrefix,
            b'^' => AttrOp::StartsWith,
            b'$' => AttrOp::EndsWith,
            b'*' => AttrOp::Includes,
            _    => AttrOp::Equals,
        };
        let value = strip_quotes(inner[pos + 2..].trim());
        AttrSel { name, op, value }
    } else {
        AttrSel { name: inner, op: AttrOp::Exists, value: "" }
    }
}

fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[s.len() - 1] == b'"') ||
           (b[0] == b'\'' && b[s.len() - 1] == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

// ── Simple (compound) selector ────────────────────────────────────────────────

struct SimpleSelector<'a> {
    tag: Option<&'a str>,
    id: Option<&'a str>,
    classes: Vec<&'a str>,
    attrs: Vec<AttrSel<'a>>,
}

fn parse_simple(s: &str) -> SimpleSelector<'_> {
    let mut tag = None;
    let mut id = None;
    let mut classes = Vec::new();
    let mut attrs = Vec::new();
    let mut rest = s.trim();

    // Optional leading tag name (not '.', '#', '[', '*')
    if rest.starts_with('*') {
        rest = &rest[1..];
    } else if !rest.starts_with(['.', '#', '[']) && !rest.is_empty() {
        let end = rest.find(['.', '#', '[']).unwrap_or(rest.len());
        if end > 0 {
            tag = Some(&rest[..end]);
            rest = &rest[end..];
        }
    }

    // #id / .class / [attr...] tokens
    while !rest.is_empty() {
        match rest.as_bytes().first() {
            Some(b'#') => {
                let end = rest[1..].find(['.', '#', '[']).map(|i| i + 1).unwrap_or(rest.len());
                id = Some(&rest[1..end]);
                rest = &rest[end..];
            }
            Some(b'.') => {
                let end = rest[1..].find(['.', '#', '[']).map(|i| i + 1).unwrap_or(rest.len());
                classes.push(&rest[1..end]);
                rest = &rest[end..];
            }
            Some(b'[') => {
                // Find closing ']', skipping quoted content
                let close = find_attr_close(&rest[1..]);
                let inner = &rest[1..close + 1]; // content between [ and ]
                attrs.push(parse_attr_sel(inner));
                rest = &rest[close + 2..]; // skip past ']'
            }
            _ => break,
        }
    }

    SimpleSelector { tag, id, classes, attrs }
}

/// Find the position of the closing `]` in `s` (which starts after the `[`).
/// Skips over quoted strings to handle values like `[attr="has]inside"]`.
fn find_attr_close(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b']' => return i,
            b'"' => { i += 1; while i < bytes.len() && bytes[i] != b'"' { i += 1; } }
            b'\'' => { i += 1; while i < bytes.len() && bytes[i] != b'\'' { i += 1; } }
            _ => {}
        }
        i += 1;
    }
    s.len() // malformed, treat the rest as the inner content
}

// ── Combinator / chain parsing ────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Combinator {
    Descendant, // whitespace
    Child,      // `>`
}

/// Find the end of the current compound-selector token, skipping `[...]` blocks.
fn find_compound_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'[' => {
                i += 1;
                i += find_attr_close(&s[i..]) + 1; // skip to ']'
            }
            b'>' | b' ' | b'\t' | b'\n' => return i,
            _ => {}
        }
        i += 1;
    }
    s.len()
}

fn parse_chain(selector: &str) -> Vec<(Option<Combinator>, SimpleSelector<'_>)> {
    let mut parts = Vec::new();
    let mut s = selector.trim();
    let mut first = true;
    let mut pending: Option<Combinator> = None;

    loop {
        s = s.trim_start();
        if s.is_empty() { break; }

        let end = find_compound_end(s);
        let token = &s[..end];
        if !token.is_empty() {
            let comb = if first { first = false; None } else { pending.take() };
            parts.push((comb, parse_simple(token)));
        }

        s = &s[end..];
        if s.is_empty() { break; }

        // Consume non-'>' whitespace, then check for '>'
        s = s.trim_start_matches(|c: char| c != '>' && c.is_ascii_whitespace());

        if s.starts_with('>') {
            pending = Some(Combinator::Child);
            s = &s[1..];
        } else if !s.is_empty() {
            pending = Some(Combinator::Descendant);
        }
    }

    parts
}

// ── Matching ──────────────────────────────────────────────────────────────────

fn matches_simple(data: &NodeData, sel: &SimpleSelector<'_>) -> bool {
    let Some(elem) = data.downcast_element() else { return false };

    if let Some(tag) = sel.tag {
        if !elem.name.local.as_ref().eq_ignore_ascii_case(tag) { return false; }
    }

    if let Some(id) = sel.id {
        if elem.attr("id") != Some(id) { return false; }
    }

    for cls in &sel.classes {
        let has = elem.attr("class")
            .map(|v| v.split_ascii_whitespace().any(|t| t == *cls))
            .unwrap_or(false);
        if !has { return false; }
    }

    for a in &sel.attrs {
        let val = elem.attr(a.name);
        let ok = match a.op {
            AttrOp::Exists => val.is_some(),
            AttrOp::Equals => val == Some(a.value),
            AttrOp::Contains => val
                .map(|v| v.split_ascii_whitespace().any(|t| t == a.value))
                .unwrap_or(false),
            AttrOp::DashPrefix => val
                .map(|v| v == a.value || (v.starts_with(a.value) && v[a.value.len()..].starts_with('-')))
                .unwrap_or(false),
            AttrOp::StartsWith => val.map(|v| v.starts_with(a.value)).unwrap_or(false),
            AttrOp::EndsWith   => val.map(|v| v.ends_with(a.value)).unwrap_or(false),
            AttrOp::Includes   => val.map(|v| v.contains(a.value)).unwrap_or(false),
        };
        if !ok { return false; }
    }

    true
}

fn node_matches_chain(
    doc: &Document,
    node_id: usize,
    parts: &[(Option<Combinator>, SimpleSelector<'_>)],
) -> bool {
    let Some(last_idx) = parts.len().checked_sub(1) else { return true };
    let (combinator_to_here, last_sel) = &parts[last_idx];

    let Some(node) = doc.get(node_id) else { return false };
    if !matches_simple(&node.data, last_sel) { return false; }
    if last_idx == 0 { return true; }

    let remaining = &parts[..last_idx];

    match combinator_to_here {
        Some(Combinator::Child) => {
            let Some(parent_id) = node.parent else { return false };
            node_matches_chain(doc, parent_id, remaining)
        }
        Some(Combinator::Descendant) | None => {
            let mut cur = node.parent;
            while let Some(ancestor_id) = cur {
                if node_matches_chain(doc, ancestor_id, remaining) { return true; }
                cur = doc.get(ancestor_id).and_then(|n| n.parent);
            }
            false
        }
    }
}

// ── Document query methods ────────────────────────────────────────────────────

impl Document {
    pub fn query_selector(&self, scope: usize, selector: &str) -> Option<usize> {
        let parts = parse_chain(selector);
        if parts.is_empty() { return None; }
        self.dfs_find_chain(scope, &parts)
    }

    fn dfs_find_chain(
        &self,
        node_id: usize,
        parts: &[(Option<Combinator>, SimpleSelector<'_>)],
    ) -> Option<usize> {
        let node = self.get(node_id)?;
        for &child in &node.children {
            if node_matches_chain(self, child, parts) { return Some(child); }
            if let Some(found) = self.dfs_find_chain(child, parts) { return Some(found); }
        }
        None
    }

    pub fn query_selector_all(&self, scope: usize, selector: &str) -> Vec<usize> {
        let parts = parse_chain(selector);
        let mut results = Vec::new();
        if !parts.is_empty() {
            self.dfs_collect_chain(scope, &parts, &mut results);
        }
        results
    }

    fn dfs_collect_chain(
        &self,
        node_id: usize,
        parts: &[(Option<Combinator>, SimpleSelector<'_>)],
        out: &mut Vec<usize>,
    ) {
        let Some(node) = self.get(node_id) else { return };
        for &child in &node.children {
            if node_matches_chain(self, child, parts) { out.push(child); }
            self.dfs_collect_chain(child, parts, out);
        }
    }
}
