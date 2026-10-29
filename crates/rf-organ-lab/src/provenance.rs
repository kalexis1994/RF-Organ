// SPDX-License-Identifier: GPL-2.0-or-later
//! Writes the engine's provenance table out as `docs/PROVENANCE.md`.

use rf_organ_dsp::provenance::{Basis, CONSTANTS, NOT_MODEL, Span, TABLES};
use std::fmt::Write as _;

/// The page, exactly as it is committed.
pub fn markdown() -> String {
    let mut page = String::new();
    page.push_str(
        "# Provenance\n\
         \n\
         Every number the engine is built from, what it rests on and the span it\n\
         could plausibly take. Generated from `crates/rf-organ-dsp/src/provenance.rs`\n\
         by `cargo run -p rf-organ-lab -- provenance docs/PROVENANCE.md`; a test\n\
         holds this page to that table, so edit the table and regenerate.\n\
         \n\
         | Basis | Meaning |\n\
         |---|---|\n\
         | documented | Stated in a manufacturer's document: a service manual, a schematic, an owner's manual or a patent. |\n\
         | measured | Measured on real instruments by someone else, and published. |\n\
         | derived | Follows by physics or arithmetic from documented or measured figures. |\n\
         | borrowed | Taken from another model rather than from an instrument. |\n\
         | provisional | Chosen here, pending a measurement. |\n\
         \n\
         A span is *sourced* when a document or a measurement gives it and *assumed*\n\
         when nothing published bounds it; an assumed span says where a sensitivity\n\
         survey should look, not where the truth is.\n\
         \n\
         ## Summary\n\
         \n\
         | Basis | Constants |\n\
         |---|---:|\n",
    );
    for basis in Basis::ALL {
        let count = CONSTANTS
            .iter()
            .filter(|constant| constant.basis == basis)
            .count();
        let _ = writeln!(page, "| {} | {count} |", basis.name());
    }
    let _ = writeln!(page, "| **total** | **{}** |", CONSTANTS.len());

    let mut module = "";
    for constant in CONSTANTS {
        let (this_module, name) = constant
            .name
            .split_once("::")
            .unwrap_or(("", constant.name));
        if this_module != module {
            module = this_module;
            let _ = write!(
                page,
                "\n## `{module}`\n\n\
                 | Constant | Value | Unit | Basis | Span | Rests on |\n\
                 |---|---:|---|---|---|---|\n"
            );
        }
        let _ = writeln!(
            page,
            "| `{name}` | {} | {} | {} | {} | {} |",
            number(constant.value),
            constant.unit,
            constant.basis.name(),
            span(constant.span),
            constant.note
        );
    }

    page.push_str("\n## Tables\n\n| Table | Basis | Rests on |\n|---|---|---|\n");
    for table in TABLES {
        let _ = writeln!(
            page,
            "| `{}` | {} | {} |",
            table.name,
            table.basis.name(),
            table.note
        );
    }

    page.push_str(
        "\n## Not part of the model\n\n\
         Scalars the engine declares that describe no instrument.\n\n\
         | Constant | Why |\n|---|---|\n",
    );
    for (name, reason) in NOT_MODEL {
        let _ = writeln!(page, "| `{name}` | {reason} |");
    }
    page
}

/// A value as its constant was written: most are `f32`, and printing them
/// through `f64` would show the rounding of the conversion rather than the
/// figure.
fn number(value: f64) -> String {
    let narrow = value as f32;
    if f64::from(narrow) == value {
        format!("{narrow}")
    } else {
        format!("{value}")
    }
}

fn span(span: Span) -> String {
    match span {
        Span::Exact => "exact".to_owned(),
        Span::Sourced(low, high) => format!("{} to {} (sourced)", number(low), number(high)),
        Span::Assumed(low, high) => format!("{} to {} (assumed)", number(low), number(high)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page is read by people deciding what to measure next, and a page
    /// that disagrees with the engine misleads them.
    #[test]
    fn the_committed_page_is_the_table() {
        let committed = include_str!("../../../docs/PROVENANCE.md");
        assert!(
            committed == markdown(),
            "docs/PROVENANCE.md is stale: run `cargo run -p rf-organ-lab -- provenance docs/PROVENANCE.md`"
        );
    }

    #[test]
    fn values_print_as_they_were_written() {
        assert_eq!(number(f64::from(0.0015_f32)), "0.0015");
        assert_eq!(number(7117.6259), "7117.6259");
        assert_eq!(number(48.0), "48");
    }
}
