//! A deliberately tiny front-matter format: `key: value` lines fenced by `---`.
//!
//! This is NOT YAML. We only support the handful of flat string fields an issue
//! needs, which lets us avoid pulling in a YAML parser as a dependency.

/// An issue document: an ordered list of front-matter fields plus the body.
pub struct Document {
    /// (key, value) pairs, order preserved.
    pub fields: Vec<(String, String)>,
    /// Everything after the closing `---`.
    pub body: String,
}

impl Document {
    /// Parse a raw file. If it has no leading `---` fence, the whole text is
    /// treated as the body with empty front-matter.
    pub fn parse(raw: &str) -> Document {
        let mut lines = raw.lines();
        // Must open with a `---` fence (allow a leading BOM/whitespace-free line).
        let first = lines.clone().next().map(|l| l.trim_end());
        if first != Some("---") {
            return Document {
                fields: Vec::new(),
                body: raw.to_string(),
            };
        }
        lines.next(); // consume opening fence

        let mut fields = Vec::new();
        let mut closed = false;
        let mut consumed = 1; // opening fence
        for line in lines.by_ref() {
            consumed += 1;
            if line.trim_end() == "---" {
                closed = true;
                break;
            }
            if let Some((k, v)) = line.split_once(':') {
                fields.push((k.trim().to_string(), v.trim().to_string()));
            }
        }

        if !closed {
            // Malformed: no closing fence. Treat original as body.
            return Document {
                fields: Vec::new(),
                body: raw.to_string(),
            };
        }

        // Reconstruct the body: skip the front-matter region, drop one blank
        // separator line if present.
        let body: String = {
            let mut rest: Vec<&str> = raw.lines().skip(consumed).collect();
            if rest.first().is_some_and(|l| l.trim().is_empty()) {
                rest.remove(0);
            }
            rest.join("\n")
        };

        Document { fields, body }
    }

    /// Get a field value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Set (or insert) a field value, preserving position of existing keys.
    pub fn set(&mut self, key: &str, value: &str) {
        if let Some(entry) = self.fields.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value.to_string();
        } else {
            self.fields.push((key.to_string(), value.to_string()));
        }
    }

    /// Render back to the on-disk representation.
    pub fn render(&self) -> String {
        let mut out = String::from("---\n");
        for (k, v) in &self.fields {
            out.push_str(k);
            out.push_str(": ");
            out.push_str(v);
            out.push('\n');
        }
        out.push_str("---\n\n");
        out.push_str(self.body.trim_end());
        out.push('\n');
        out
    }
}
