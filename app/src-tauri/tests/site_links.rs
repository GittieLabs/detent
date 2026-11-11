//! The documentation site (`site/`, M9): every page is in the table of
//! contents, and every link between pages, and to a heading on one, goes
//! somewhere. `mdbook build site` makes the HTML; this needs no network and
//! no mdBook, so it runs with the other tests.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../site/src")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(src().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pages() -> BTreeSet<String> {
    std::fs::read_dir(src())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".md") && n != "SUMMARY.md")
        .collect()
}

/// Every `](target)` in a page.
fn links(text: &str) -> Vec<String> {
    text.split("](")
        .skip(1)
        .filter_map(|rest| rest.split(')').next())
        .map(str::to_owned)
        .collect()
}

/// mdBook's anchor for a heading: lowercase, spaces to `-`, most
/// punctuation dropped.
fn anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

fn anchors(page: &str) -> BTreeSet<String> {
    read(page)
        .lines()
        .filter_map(|l| l.strip_prefix('#'))
        .map(|l| anchor(l.trim_start_matches('#')))
        .collect()
}

#[test]
fn every_page_is_in_the_table_of_contents_and_nothing_else_is() {
    let summary = read("SUMMARY.md");
    let listed: BTreeSet<String> = links(&summary).into_iter().collect();
    assert_eq!(listed, pages());
}

#[test]
fn every_link_between_pages_goes_somewhere() {
    let pages = pages();
    for page in &pages {
        for link in links(&read(page)) {
            if link.starts_with("http://") || link.starts_with("https://") {
                continue;
            }
            let (file, frag) = match link.split_once('#') {
                Some((f, a)) => (if f.is_empty() { page.as_str() } else { f }, Some(a)),
                None => (link.as_str(), None),
            };
            assert!(pages.contains(file), "{page}: link to missing page {link}");
            if let Some(a) = frag {
                assert!(
                    anchors(file).contains(a),
                    "{page}: no heading #{a} in {file}"
                );
            }
        }
    }
}

#[test]
fn the_dashboard_default_points_at_this_site() {
    let book = std::fs::read_to_string(src().join("../book.toml")).unwrap();
    assert!(book.contains("site-url = \"/detent/\""), "{book}");
    assert_eq!(
        detent_lib::store::DEFAULT_DASHBOARD_URL,
        "https://gittielabs.github.io/detent/"
    );
}
