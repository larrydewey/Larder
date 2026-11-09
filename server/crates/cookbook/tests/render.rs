mod support;

use std::time::Instant;

use larder_cookbook::{render_data_json, render_pdf, themes, BookRecipe, CookbookOptions, GroupBy};
use larder_core::units::UnitSystem;
use larder_core::{Ingredient, Instruction, RecipeData};
use support::sample_data::{sample_recipes, synthetic_recipes};

fn assert_pdf(pdf: &[u8], min_len: usize) {
    assert!(pdf.starts_with(b"%PDF"), "output is not a PDF");
    assert!(pdf.len() > min_len, "PDF suspiciously small: {} bytes", pdf.len());
    assert!(pdf.windows(5).rev().take(64).any(|w| w == b"%%EOF"), "PDF is not terminated");
}

/// Number of pages, counted from the page objects.
fn page_count(pdf: &[u8]) -> usize {
    let needle = b"/Type /Page";
    let alt = b"/Type/Page";
    let count =
        |n: &[u8]| pdf.windows(n.len()).enumerate().filter(|(i, w)| *w == n && pdf.get(i + n.len()).is_none_or(|c| *c != b's')).count();
    count(needle) + count(alt)
}

#[test]
fn every_theme_and_paper_renders() {
    let with_images = sample_recipes(true);
    let without_images = sample_recipes(false);
    for theme in themes() {
        for paper in ["letter", "a4", "a5"] {
            let opts = CookbookOptions {
                title: "Test Kitchen".into(),
                subtitle: Some("Recipes for testing".into()),
                author: Some("QA".into()),
                dedication: Some("For everyone who tests.".into()),
                theme: theme.id.into(),
                paper: paper.into(),
                include_nutrition: true,
                ..Default::default()
            };
            let pdf = render_pdf(&opts, with_images.clone()).unwrap_or_else(|e| panic!("{} {paper}: {e:#}", theme.id));
            assert_pdf(&pdf, 50_000);
            let plain = render_pdf(&CookbookOptions { include_images: false, ..opts.clone() }, without_images.clone())
                .unwrap_or_else(|e| panic!("{} {paper} (no images): {e:#}", theme.id));
            assert_pdf(&plain, 20_000);
            assert!(pdf.len() > plain.len(), "photos should make the PDF larger");
        }
    }
}

#[test]
fn options_variants_render() {
    let recipes = sample_recipes(false);
    for (group_by, toc, index, units) in [
        (GroupBy::Tag, false, true, UnitSystem::Metric),
        (GroupBy::None, true, false, UnitSystem::Imperial),
        (GroupBy::Category, false, false, UnitSystem::Original),
    ] {
        let opts = CookbookOptions {
            group_by,
            include_toc: toc,
            include_index: index,
            include_notes: false,
            units,
            theme: "does-not-exist".into(),
            paper: "tabloid".into(),
            ..Default::default()
        };
        let pdf = render_pdf(&opts, recipes.clone()).unwrap();
        assert_pdf(&pdf, 10_000);
    }
}

#[test]
fn empty_cookbook_is_still_a_pdf() {
    for theme in themes() {
        let opts = CookbookOptions { theme: theme.id.into(), ..Default::default() };
        let pdf = render_pdf(&opts, vec![]).unwrap();
        assert_pdf(&pdf, 5_000);
        assert!(page_count(&pdf) >= 2, "title page + empty note");
    }
    let opts = CookbookOptions { title: "   ".into(), dedication: Some("".into()), ..Default::default() };
    assert_pdf(&render_pdf(&opts, vec![]).unwrap(), 5_000);
}

#[test]
fn recipe_without_content_renders() {
    let r = BookRecipe { id: "x".into(), data: RecipeData::default(), image: Some(vec![1, 2, 3]) };
    let pdf = render_pdf(&CookbookOptions::default(), vec![r]).unwrap();
    assert_pdf(&pdf, 5_000);
}

#[test]
fn malicious_text_is_rendered_literally() {
    let evil = [
        r#"#panic("x")"#,
        "]",
        "[",
        "*bold* _emph_",
        "$x^2$",
        r"\",
        "<label>",
        "@ref",
        "#{ 1 / 0 }",
        "// comment",
        "/* open comment",
        "```raw",
        "= Heading",
        r#"#import "/data.json""#,
        r#"#read("/etc/passwd")"#,
        "#set page(width: 1pt)",
        "}{",
        "\"",
        "\u{0}\u{7}\u{feff}",
    ];
    let joined = evil.join(" ");
    let mut nutrition = std::collections::BTreeMap::new();
    nutrition.insert("#panic(\"k\")".to_string(), "$ ] [".to_string());
    let data = RecipeData {
        name: joined.clone(),
        description: format!("{joined}\n\n{joined}"),
        notes: joined.clone(),
        yield_quantity: Some(2.0),
        yield_unit: "#panic(\"unit\")".into(),
        source_name: Some(joined.clone()),
        source_url: Some("https://evil.example.com/\"]#panic(\"u\")".into()),
        categories: vec!["#panic(\"cat\") ] <x>".into()],
        tags: vec!["@ref $".into(), "*".into()],
        ingredients: evil
            .iter()
            .map(|e| Ingredient { section: Some((*e).into()), ..Ingredient::parse(&format!("2 cups {e}")) })
            .chain([Ingredient { original: joined.clone(), ..Default::default() }])
            .collect(),
        instructions: evil.iter().map(|e| Instruction { section: Some((*e).into()), text: (*e).into() }).collect(),
        nutrition,
        ..Default::default()
    };
    let recipe = BookRecipe { id: "evil".into(), data, image: None };
    for theme in themes() {
        let opts = CookbookOptions {
            title: joined.clone(),
            subtitle: Some(joined.clone()),
            author: Some(joined.clone()),
            dedication: Some(joined.clone()),
            chapter_order: vec![joined.clone()],
            theme: theme.id.into(),
            include_nutrition: true,
            ..Default::default()
        };
        let pdf = render_pdf(&opts, vec![recipe.clone()]).unwrap_or_else(|e| panic!("{}: {e:#}", theme.id));
        assert_pdf(&pdf, 5_000);
    }
    // The text reaches the template verbatim (as JSON data, not markup).
    let json = render_data_json(&CookbookOptions::default(), &[recipe]);
    assert!(json.contains("#panic(“x”) ] [ *bold* _emph_ $x^2$"), "{json}");
    assert!(json.contains("<label> @ref #{ 1 / 0 }"));
}

#[test]
fn many_recipes_render_in_reasonable_time() {
    let n: usize = std::env::var("COOKBOOK_PERF_RECIPES").ok().and_then(|s| s.parse().ok()).unwrap_or(200);
    let recipes = synthetic_recipes(n, 5);
    let opts = CookbookOptions { title: "Big Book".into(), ..Default::default() };
    let started = Instant::now();
    let pdf = render_pdf(&opts, recipes).unwrap();
    let elapsed = started.elapsed();
    let pages = page_count(&pdf);
    eprintln!("rendered {n} recipes -> {pages} pages, {} KB in {elapsed:.2?}", pdf.len() / 1024);
    assert_pdf(&pdf, 100_000);
    assert!(pages > n, "expected at least one page per recipe, got {pages}");
    // Generous bound so slow CI machines and debug builds pass; release builds
    // are far faster (see the crate docs / report).
    assert!(elapsed.as_secs() < 240, "rendering took {elapsed:.2?}");
}
