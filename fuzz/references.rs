//! The Java reference registry, `scripts/references.tsv` (docs/PORTING.md §9), embedded at build
//! time from this checkout, so a branch decides its default. `$CLOSURE_RS_REF` selects a row by
//! tag; without it the registry's `default` row applies. Shared by the fuzz crates with
//! `#[path = "../../references.rs"] pub mod references;` (the mutator stays oracle-free).

/// `scripts/references.tsv` of this checkout.
pub const REGISTRY: &str = include_str!("../scripts/references.tsv");

/// One registry row. Paths are relative to the repository root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub tag: String,
    pub commit: String,
    /// `-` until the reference jar is built and pinned.
    pub jar_sha256: String,
    pub src: String,
    pub recording_ws: String,
    pub jar: String,
    pub oracle_jar: String,
}

impl Reference {
    /// The D2 golden store tag, `ref-<first 8 hex digits of the jar sha256>`; `None` while the
    /// jar is not pinned.
    pub fn golden_tag(&self) -> Option<String> {
        (self.jar_sha256 != "-").then(|| format!("ref-{}", &self.jar_sha256[..8]))
    }
}

/// The row `$CLOSURE_RS_REF` names (default: the registry's `default` row) of [`REGISTRY`].
pub fn reference() -> Result<Reference, String> {
    let tag = std::env::var("CLOSURE_RS_REF")
        .ok()
        .filter(|t| !t.is_empty());
    reference_in(REGISTRY, tag.as_deref())
}

/// The row `tag` (default: the `default` row) of the registry text `registry`.
pub fn reference_in(registry: &str, tag: Option<&str>) -> Result<Reference, String> {
    let rows: Vec<Vec<&str>> = registry
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').collect())
        .collect();
    let default = rows
        .iter()
        .find(|c| c[0] == "default" && c.len() >= 2)
        .map(|c| c[1]);
    let tag = tag
        .or(default)
        .ok_or("scripts/references.tsv has no default row")?;
    let c = rows
        .iter()
        .find(|c| c.len() >= 7 && c[0] == tag)
        .ok_or_else(|| {
            format!("unknown reference '{tag}' (CLOSURE_RS_REF; tags in scripts/references.tsv)")
        })?;
    Ok(Reference {
        tag: c[0].into(),
        commit: c[1].into(),
        jar_sha256: c[2].into(),
        src: c[3].into(),
        recording_ws: c[4].into(),
        jar: c[5].into(),
        oracle_jar: c[6].into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_row_and_tags() {
        let r = reference_in(REGISTRY, None).unwrap();
        let d = reference_in(REGISTRY, Some(&r.tag)).unwrap();
        assert_eq!(r, d);
        let reg = "# c\nold\taaaa\t0123456789ab\tr/o\tr/o-rec\tb/o.jar\tb/oo.jar\n\
                   new\tbbbb\t-\tr/n\tr/n-rec\tb/n.jar\tb/no.jar\ndefault\told\n";
        let old = reference_in(reg, None).unwrap();
        assert_eq!(old.jar, "b/o.jar");
        assert_eq!(old.golden_tag().as_deref(), Some("ref-01234567"));
        let new = reference_in(reg, Some("new")).unwrap();
        assert_eq!(
            (new.oracle_jar.as_str(), new.golden_tag()),
            ("b/no.jar", None)
        );
        assert!(reference_in(reg, Some("nope")).is_err());
    }
}
