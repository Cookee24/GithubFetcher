use std::{borrow::Cow, fmt};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoArgs {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StatsArgs {
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub r#ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TreeArgs {
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default = "default_depth")]
    pub depth: usize,
    pub r#ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFileArgs {
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub r#ref: Option<String>,
    #[serde(default)]
    pub line_range: Option<LineRange>,
    #[serde(default)]
    pub max_chars: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepoInfo {
    pub description: Option<String>,
    pub stars: u64,
    pub forks: u64,
    pub license: Option<LicenseInfo>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RepoResponse {
    pub repo: Option<RepoInfo>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LicenseInfo {
    pub key: Option<String>,
    pub name: Option<String>,
    pub spdx_id: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TreeEntry {
    pub r#type: EntryType,
    /// Base name of the entry (no parent path).
    pub name: String,
    /// Full path relative to the repository root, kept internal for tree assembly.
    #[serde(skip)]
    #[schemars(skip)]
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodule_git_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeEntry>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TreeResponse {
    pub entries: Vec<TreeEntry>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StatsResponse {
    pub item: Stats,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    File,
    Dir,
    Symlink,
    Submodule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineRange {
    /// A single number N or a prefix like `..N` means keep lines 1..=N.
    End(usize),
    /// A range like `start..end` or `start...end` keeps lines start..=end.
    Range { start: usize, end: usize },
    /// A suffix like `start..` keeps lines start..=EOF.
    Start(usize),
}

impl LineRange {
    pub fn bounds(self) -> (usize, Option<usize>) {
        match self {
            LineRange::End(end) => (1, Some(end)),
            LineRange::Range { start, end } => (start, Some(end)),
            LineRange::Start(start) => (start, None),
        }
    }
}

impl Serialize for LineRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LineRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LineRangeVisitor;

        impl<'de> Visitor<'de> for LineRangeVisitor {
            type Value = LineRange;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a line range like \"1..200\", \"1...200\", \"..200\", \"1..\", \"1:200\", \"1:\", \":200\", or a positive integer")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(LineRange::End(value as usize))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value < 0 {
                    return Err(E::invalid_value(Unexpected::Signed(value), &self));
                }

                Ok(LineRange::End(value as usize))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                parse_line_range(value)
                    .ok_or_else(|| E::invalid_value(Unexpected::Str(value), &self))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&value)
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let _ = seq;
                Err(de::Error::invalid_type(Unexpected::Seq, &self))
            }
        }

        deserializer.deserialize_any(LineRangeVisitor)
    }
}

impl fmt::Display for LineRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineRange::End(end) => write!(f, "..{}", end),
            LineRange::Range { start, end } => write!(f, "{}..{}", start, end),
            LineRange::Start(start) => write!(f, "{}..", start),
        }
    }
}

impl JsonSchema for LineRange {
    fn schema_name() -> Cow<'static, str> {
        Cow::from("LineRange")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": ["string", "integer"],
            "description": "Line ranges like \"1..200\", \"1...200\", \"1..\", \"..200\", \"1:200\", \"1:\", or \":200\"; a bare number N keeps lines 1..=N.",
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FileResponse {
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Stats {
    #[serde(rename = "type")]
    pub r#type: EntryType,
    /// Base name of the entry (no parent path).
    pub name: String,
    /// Full path relative to the repository root.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodule_git_url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArgs {
    pub query: String,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListReposArgs {
    pub owner: String,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SearchResult {
    pub name: String,
    pub path: String,
    pub repository: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepoSummary {
    pub name: String,
    pub full_name: String,
    pub private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReposResponse {
    pub repos: Vec<RepoSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TagsResponse {
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BranchesResponse {
    pub branches: Vec<String>,
}

pub fn default_depth() -> usize {
    1
}

fn parse_line_range(value: &str) -> Option<LineRange> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(end) = value.parse::<usize>() {
        return Some(LineRange::End(end));
    }

    let separator = if value.contains("...") {
        "..."
    } else if value.contains("..") {
        ".."
    } else if value.contains(':') {
        ":"
    } else {
        return None;
    };

    let mut parts = value.splitn(2, separator);
    let start = parts.next().unwrap_or("").trim();
    let end = parts.next().unwrap_or("").trim();

    let start = if start.is_empty() {
        None
    } else {
        start.parse::<usize>().ok()
    };

    let end = if end.is_empty() {
        None
    } else {
        end.parse::<usize>().ok()
    };

    match (start, end) {
        (Some(start), Some(end)) => Some(LineRange::Range { start, end }),
        (Some(start), None) => Some(LineRange::Start(start)),
        (None, Some(end)) => Some(LineRange::End(end)),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::LineRange;

    #[test]
    fn parses_line_range_strings() {
        let cases = [
            ("\"1..200\"", LineRange::Range { start: 1, end: 200 }),
            ("\"1...200\"", LineRange::Range { start: 1, end: 200 }),
            ("\"..200\"", LineRange::End(200)),
            ("\"1..\"", LineRange::Start(1)),
            ("\"1:200\"", LineRange::Range { start: 1, end: 200 }),
            ("\"1:\"", LineRange::Start(1)),
            ("\":200\"", LineRange::End(200)),
        ];

        for (input, expected) in cases {
            let parsed: LineRange = serde_json::from_str(input).unwrap();
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn parses_numeric_line_range() {
        let parsed: LineRange = serde_json::from_str("10").unwrap();
        assert_eq!(parsed, LineRange::End(10));
    }
}
