//! Writes sample cookbooks for every theme to a directory.
//!
//! ```sh
//! cargo run --release -p larder-cookbook --example sample -- /tmp/cookbooks [paper] [synthetic-count]
//! ```

#[path = "../tests/support/sample_data.rs"]
mod sample_data;

use std::path::PathBuf;
use std::time::Instant;

use larder_cookbook::{render_pdf, themes, CookbookOptions, GroupBy};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "cookbook-samples".into()));
    let paper = args.next().unwrap_or_else(|| "letter".into());
    let synthetic: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    std::fs::create_dir_all(&dir)?;

    for theme in themes() {
        let opts = CookbookOptions {
            title: "The Hartley Family Kitchen".into(),
            subtitle: Some("Recipes gathered around our table, 1962–2026".into()),
            author: Some("The Hartley Family".into()),
            dedication: Some("For Grandma Rose,\nwho never measured anything\nand never got it wrong.".into()),
            theme: theme.id.into(),
            paper: paper.clone(),
            group_by: GroupBy::Category,
            chapter_order: vec!["Breakfast".into(), "Soups".into(), "Salads".into(), "Mains".into()],
            include_nutrition: true,
            ..Default::default()
        };
        let mut recipes = sample_data::sample_recipes(true);
        if synthetic > 0 {
            recipes.extend(sample_data::synthetic_recipes(synthetic, 4));
        }
        let started = Instant::now();
        let pdf = render_pdf(&opts, recipes)?;
        let path = dir.join(format!("cookbook-{}-{}.pdf", theme.id, paper));
        std::fs::write(&path, &pdf)?;
        println!("{} ({} KB) in {:.2?}", path.display(), pdf.len() / 1024, started.elapsed());
    }
    Ok(())
}
