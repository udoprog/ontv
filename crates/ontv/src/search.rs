use std::collections::HashSet;

/// A bucket of tokens.
pub(crate) struct Tokens {
    tokens: HashSet<String>,
}

impl Tokens {
    /// Construct a new set of tokens.
    pub(crate) fn new(string: &str) -> Self {
        let mut tokens = HashSet::new();
        tokenize(string, &mut tokens);
        Self { tokens }
    }

    /// Test if the tokenization is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Test if tokens matches the given string.
    pub(crate) fn matches(&self, string: &str) -> bool {
        let mut search = HashSet::new();
        tokenize_search(string, &mut search);
        self.tokens.iter().all(|t| search.contains(t.as_str()))
    }
}

/// Tokenize a string for filtering.
fn tokenize(input: &str, output: &mut HashSet<String>) {
    let mut string = String::new();

    for part in input.split_whitespace() {
        string.clear();

        for c in part.chars().filter(|c| c.is_alphanumeric()) {
            string.extend(c.to_lowercase());
        }

        output.insert(string.clone());
    }
}

/// Tokenize a string for searching which includes prefixes.
fn tokenize_search(input: &str, output: &mut HashSet<String>) {
    let mut string = String::new();

    for part in input.split_whitespace() {
        string.clear();

        for c in part.chars().filter(|c| c.is_alphanumeric()) {
            string.extend(c.to_lowercase());
            output.insert(string.clone());
        }
    }
}
