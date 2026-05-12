use std::collections::HashMap;

pub(super) struct Template {
    vars: HashMap<String, String>,
}

impl Template {
    pub(super) fn new(vars: HashMap<String, String>) -> Self {
        Self { vars }
    }

    pub(super) fn insert(&mut self, var: &str, val: String) {
        self.vars.insert(var.to_owned(), val);
    }

    pub(super) fn resolve(&self, template: &str) -> String {
        let mut result = template.to_owned();

        for (var, value) in &self.vars {
            // Simple form: $Number$
            let simple = format!("${var}$");
            result = result.replace(&simple, value);

            // Zero-padded form: $Number%0Xd$ where X is a single digit (1-9)
            let prefix = format!("${var}%0");
            if let Some(pos) = result.find(&prefix) {
                let rest = &result.as_bytes()[pos + prefix.len()..];
                if rest.len() >= 3
                    && rest[0].is_ascii_digit()
                    && rest[1] == b'd'
                    && rest[2] == b'$'
                {
                    let width = (rest[0] - b'0') as usize;
                    let padded = format!("{value:0>width$}");
                    let pattern_end = pos + prefix.len() + 3;
                    result = format!("{}{padded}{}", &result[..pos], &result[pattern_end..]);
                }
            }
        }

        result
    }
}
