// Theme and paper parameters for the Larder cookbook template.
// Components in cookbook.typ read these values and branch on `th.id` where a
// theme needs a genuinely different construction (not just different values).

#let papers = (
  letter: (
    name: "us-letter",
    margin: (top: 0.9in, bottom: 0.85in, inside: 0.95in, outside: 0.8in),
    scale: 1.0,
    stacked: false,
    photo-height: 2.9in,
    columns: 3,
  ),
  a4: (
    name: "a4",
    margin: (top: 23mm, bottom: 22mm, inside: 24mm, outside: 20mm),
    scale: 1.0,
    stacked: false,
    photo-height: 76mm,
    columns: 3,
  ),
  a5: (
    name: "a5",
    margin: (top: 17mm, bottom: 17mm, inside: 17mm, outside: 13mm),
    scale: 0.88,
    stacked: true,
    photo-height: 52mm,
    columns: 2,
  ),
)

#let themes = (
  classic: (
    id: "classic",
    body-font: ("EB Garamond", "Libertinus Serif", "DejaVu Sans Mono"),
    display-font: ("EB Garamond", "Libertinus Serif", "DejaVu Sans Mono"),
    label-font: ("EB Garamond", "Libertinus Serif", "DejaVu Sans Mono"),
    ornament-font: ("EB Garamond", "Libertinus Serif", "DejaVu Sans Mono"),
    body-size: 11pt,
    leading: 0.62em,
    justify: true,
    ink: rgb("#2b2522"),
    accent: rgb("#7b1e2b"),
    muted: rgb("#6e625a"),
    rule: rgb("#b9a99a"),
    tint: rgb("#f6f0e8"),
    fleuron: "❦",
    fleuron-alt: "❧",
    align: center,
    ing-width: 34%,
  ),
  modern: (
    id: "modern",
    body-font: ("Source Sans 3", "EB Garamond", "Libertinus Serif", "DejaVu Sans Mono"),
    display-font: ("DM Serif Display", "EB Garamond", "Libertinus Serif"),
    label-font: ("Source Sans 3", "Libertinus Serif"),
    ornament-font: ("Source Sans 3", "Libertinus Serif"),
    body-size: 10pt,
    leading: 0.68em,
    justify: false,
    ink: rgb("#1f1d1b"),
    accent: rgb("#b5532f"),
    muted: rgb("#6d6862"),
    rule: rgb("#e2d8d1"),
    tint: rgb("#f7efea"),
    fleuron: "◆",
    fleuron-alt: "◆",
    align: left,
    ing-width: 33%,
  ),
  rustic: (
    id: "rustic",
    body-font: ("Lora", "EB Garamond", "Libertinus Serif", "DejaVu Sans Mono"),
    display-font: ("Caveat", "Lora", "EB Garamond", "Libertinus Serif"),
    label-font: ("Lora", "EB Garamond", "Libertinus Serif"),
    ornament-font: ("EB Garamond", "DejaVu Sans Mono"),
    body-size: 10pt,
    leading: 0.66em,
    justify: true,
    ink: rgb("#3a2a1c"),
    accent: rgb("#8b4a22"),
    muted: rgb("#7a6250"),
    rule: rgb("#a47b55"),
    tint: rgb("#f8f1e5"),
    fleuron: "❦",
    fleuron-alt: "❧",
    align: center,
    ing-width: 36%,
  ),
)

#let make-theme(id, paper) = {
  let th = themes.at(id, default: themes.classic)
  let pp = papers.at(paper, default: papers.letter)
  let s = pp.scale
  th + (paper: pp, s: s, size: th.body-size * s)
}
