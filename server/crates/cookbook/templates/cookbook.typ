// Larder cookbook template.
//
// All book content is read from /data.json (produced by the Rust side) and is
// only ever rendered as plain strings, never evaluated as markup. Photos are
// served as /img/<n>.jpg.

#import "/themes.typ": make-theme

#let data = json("/data.json")
#let th = make-theme(data.theme, data.paper)
#let pp = th.paper
#let sz(x) = x * th.s

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

#let display(body, size: 20pt, fill: none, weight: "regular", style: "normal", tracking: 0pt) = text(
  font: th.display-font,
  size: sz(size),
  fill: if fill == none { th.ink } else { fill },
  weight: weight,
  style: style,
  tracking: tracking,
  hyphenate: false,
  body,
)

// A small-capitals / spaced-capitals label in the theme's label face.
#let label-text(body, size: 9pt, fill: none, tracking: 0.12em) = {
  let fill = if fill == none { th.accent } else { fill }
  if th.id == "classic" {
    text(font: th.label-font, size: sz(size + 1.5pt), fill: fill, tracking: tracking, smallcaps(lower(body)))
  } else if th.id == "modern" {
    text(font: th.label-font, size: sz(size - 1pt), fill: fill, weight: "semibold", tracking: tracking + 0.04em, upper(body))
  } else {
    text(font: th.label-font, size: sz(size - 0.5pt), fill: fill, weight: "semibold", tracking: tracking + 0.02em, upper(body))
  }
}

#let hrule(width: 100%, stroke: 0.5pt, fill: none) = line(length: width, stroke: stroke + (if fill == none { th.rule } else { fill }))

#let double-rule(width: 100%, fill: none) = {
  let c = if fill == none { th.rule } else { fill }
  block(width: width, spacing: 0pt, {
    line(length: 100%, stroke: 1.1pt + c)
    v(2pt, weak: false)
    line(length: 100%, stroke: 0.4pt + c)
  })
}

// "——— ❦ ———"
#let fleuron-rule(width: 2.2in, glyph: none, size: 13pt) = {
  let g = if glyph == none { th.fleuron } else { glyph }
  box(width: sz(width), grid(
    columns: (1fr, auto, 1fr),
    column-gutter: 0.6em,
    align: horizon,
    line(length: 100%, stroke: 0.45pt + th.rule),
    text(font: th.ornament-font, size: sz(size), fill: th.accent, g),
    line(length: 100%, stroke: 0.45pt + th.rule),
  ))
}

#let number-words = (
  "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten", "Eleven", "Twelve",
  "Thirteen", "Fourteen", "Fifteen", "Sixteen", "Seventeen", "Eighteen", "Nineteen", "Twenty",
)

#let chapter-number(n) = {
  if th.id == "classic" { "Chapter " + numbering("I", n) }
  else if th.id == "modern" { if n < 10 { "0" + str(n) } else { str(n) } }
  else { "Chapter " + if n <= number-words.len() { number-words.at(n - 1) } else { str(n) } }
}

// Page numbers of all recipes, chapters and the index: key -> page number.
// Must be called inside `context`.
#let page-map() = {
  let m = (:)
  for a in query(<anchor>) {
    m.insert(a.value, counter(page).at(a.location()).first())
  }
  m
}

#let anchor(key) = [#metadata(key)<anchor>]

// Section marks drive running heads and folios:
//   start: first page of a section (no running head there)
//   end:   last content of a section (pages after it, before the next start, are blank versos)
#let mark-start(title, folio: true, front: false) = [#metadata((kind: "start", title: title, folio: folio, front: front))<section-mark>]
#let mark-end() = [#metadata((kind: "end"))<section-mark>]

#let page-info() = {
  let p = here().page()
  let current = none
  let starts-here = false
  let blank = false
  let prev-end = none
  for m in query(<section-mark>) {
    let mp = m.location().page()
    if m.value.kind == "start" {
      if prev-end != none and prev-end < p and p < mp { blank = true }
      if mp <= p { current = m.value }
      if mp == p { starts-here = true }
      prev-end = none
    } else {
      prev-end = mp
    }
  }
  (current: current, starts-here: starts-here, blank: blank)
}

#let is-odd-page() = calc.odd(here().page())

// ---------------------------------------------------------------------------
// Running heads and folios
// ---------------------------------------------------------------------------

#let running-head() = context {
  let info = page-info()
  if info.current == none or info.blank or info.starts-here or info.current.front { return }
  let title = info.current.title
  let odd = is-odd-page()
  let al = if odd { right } else { left }
  if th.id == "classic" {
    set align(al)
    stack(spacing: 4pt, text(font: th.label-font, size: sz(9.5pt), fill: th.muted, tracking: 0.14em, smallcaps(lower(title))), hrule(stroke: 0.35pt))
  } else if th.id == "modern" {
    set align(al)
    text(font: th.label-font, size: sz(7.5pt), fill: th.muted, weight: "semibold", tracking: 0.2em, upper(title))
  } else {
    set align(al)
    stack(spacing: 3pt, text(font: th.display-font, size: sz(15pt), fill: th.accent, weight: "bold", title), double-rule())
  }
}

#let folio() = context {
  let info = page-info()
  if info.current == none or info.blank or not info.current.folio { return }
  let n = counter(page).get().first()
  let num = if info.current.front { numbering("i", n) } else { str(n) }
  let odd = is-odd-page()
  let al = if odd { right } else { left }
  set align(al)
  if th.id == "classic" {
    text(size: sz(10pt), fill: th.ink, number-type: "old-style", num)
  } else if th.id == "modern" {
    text(font: th.display-font, size: sz(11pt), fill: th.accent, num)
  } else {
    text(font: th.display-font, size: sz(15pt), fill: th.accent, weight: "bold", "~ " + num + " ~")
  }
}

// ---------------------------------------------------------------------------
// Document setup
// ---------------------------------------------------------------------------

#set document(
  title: data.title,
  author: if data.author != none { data.author } else { () },
)

#set page(
  paper: pp.name,
  margin: pp.margin,
  binding: left,
  header: running-head(),
  footer: folio(),
  header-ascent: 38%,
  footer-descent: 40%,
)

#set text(font: th.body-font, size: th.size, fill: th.ink, lang: "en", hyphenate: auto, number-type: "lining")
#set par(justify: th.justify, leading: th.leading, spacing: th.leading + 0.35em)
#show heading: set text(hyphenate: false)

// ---------------------------------------------------------------------------
// Front matter
// ---------------------------------------------------------------------------

#let title-page() = page(header: none, footer: none, margin: pp.margin, {
  if th.id == "classic" {
    set align(center)
    // Thin double frame.
    place(center + horizon, rect(width: 100% + 0.25in, height: 100% + 0.2in, stroke: 0.6pt + th.rule, inset: 0pt,
      pad(4pt, rect(width: 100%, height: 100%, stroke: 0.3pt + th.rule))))
    v(1fr)
    text(font: th.ornament-font, size: sz(30pt), fill: th.accent, th.fleuron)
    v(sz(22pt))
    block(width: 85%, par(justify: false, leading: 0.45em, display(upper(data.title), size: 32pt, tracking: 0.06em)))
    v(sz(14pt))
    fleuron-rule(width: 2.6in, glyph: th.fleuron-alt, size: 14pt)
    if data.subtitle != none {
      v(sz(12pt))
      block(width: 80%, par(justify: false, text(size: sz(15pt), style: "italic", fill: th.muted, data.subtitle)))
    }
    v(2fr)
    if data.author != none {
      text(size: sz(13pt), tracking: 0.16em, fill: th.ink, smallcaps(lower(data.author)))
      v(sz(10pt))
    }
    text(font: th.ornament-font, size: sz(11pt), fill: th.rule, "· · ·")
    v(0.35in)
  } else if th.id == "modern" {
    // Terracotta band across the top of the page, title set in it.
    let band = 52%
    context {
      let m = pp.margin
      let left-m = if is-odd-page() { m.inside } else { m.outside }
      place(top + left, dx: -left-m, dy: -m.top,
        rect(width: page.width, height: page.height * 0.55, fill: th.accent, stroke: none))
    }
    block(height: band, width: 100%, {
      align(bottom + left, {
        text(font: th.label-font, size: sz(8.5pt), weight: "semibold", tracking: 0.32em, fill: white.transparentize(15%), upper("A Cookbook"))
        v(sz(10pt))
        block(width: 92%, par(justify: false, leading: 0.32em, display(data.title, size: 46pt, fill: white)))
        v(sz(20pt))
      })
    })
    v(sz(34pt))
    if data.subtitle != none {
      block(width: 80%, par(justify: false, leading: 0.5em, display(data.subtitle, size: 20pt, style: "italic", fill: th.ink)))
    }
    v(sz(14pt))
    box(width: 36pt, height: 3pt, fill: th.accent)
    v(1fr)
    if data.author != none {
      text(font: th.label-font, size: sz(9pt), weight: "semibold", tracking: 0.22em, fill: th.muted, upper(data.author))
    }
    v(0.2in)
  } else {
    set align(center)
    // Double-ruled frame like an old recipe box card.
    place(center + horizon, rect(width: 100% + 0.2in, height: 100% + 0.15in, stroke: 1.4pt + th.rule, inset: 0pt,
      pad(3.5pt, rect(width: 100%, height: 100%, stroke: 0.5pt + th.rule))))
    v(1fr)
    text(font: th.ornament-font, size: sz(18pt), fill: th.accent, "☙  " + th.fleuron + "  ❧")
    v(sz(16pt))
    block(width: 88%, par(justify: false, leading: 0.42em, display(data.title, size: 60pt, weight: "bold", fill: th.accent)))
    v(sz(10pt))
    box(width: 2.4in, double-rule())
    if data.subtitle != none {
      v(sz(12pt))
      block(width: 80%, par(justify: false, text(size: sz(14pt), style: "italic", fill: th.muted, data.subtitle)))
    }
    v(2fr)
    if data.author != none {
      text(size: sz(9.5pt), tracking: 0.2em, fill: th.ink, upper(data.author))
      v(sz(8pt))
    }
    v(0.3in)
  }
})

#let dedication-page() = page(header: none, footer: none, {
  set align(center)
  v(1fr)
  block(width: 70%, {
    set par(justify: false, leading: 0.7em)
    if th.id == "modern" { set align(left) }
    let lines = data.dedication.map(l => if l == "" { v(0.6em) } else { l + linebreak() }).join()
    if th.id == "rustic" {
      display(lines, size: 21pt, fill: th.accent)
    } else {
      text(size: sz(13.5pt), style: "italic", fill: th.ink, lines)
    }
  })
  v(sz(16pt))
  if th.id == "modern" { box(width: 36pt, height: 3pt, fill: th.accent) } else {
    text(font: th.ornament-font, size: sz(14pt), fill: th.accent, th.fleuron-alt)
  }
  v(2fr)
})

#let section-title(title) = {
  if th.id == "classic" {
    align(center, {
      v(sz(24pt))
      display(upper(title), size: 20pt, tracking: 0.18em)
      v(sz(4pt))
      fleuron-rule(width: 1.8in, size: 11pt)
      v(sz(22pt))
    })
  } else if th.id == "modern" {
    v(sz(18pt))
    display(title, size: 36pt)
    v(sz(-4pt))
    box(width: 36pt, height: 3pt, fill: th.accent)
    v(sz(22pt))
  } else {
    align(center, {
      v(sz(16pt))
      display(title, size: 40pt, weight: "bold", fill: th.accent)
      v(sz(-6pt))
      box(width: 2in, double-rule())
      v(sz(20pt))
    })
  }
}

#let leader() = box(width: 1fr, inset: (x: 0.35em), repeat(gap: 0.3em, text(fill: th.rule, ".")))

// A contents line: the title may wrap; the dot leader fills the rest of its
// last line and the page number sits in its own column, so long titles never
// run into the number or the neighbouring column.
#let toc-line(title, page) = grid(
  columns: (1fr, auto),
  column-gutter: 0.25em,
  align: (left + bottom, right + bottom),
  [#title#leader()],
  page,
)

#let toc-page() = {
  pagebreak(to: "odd", weak: true)
  mark-start("Contents", front: true)
  section-title("Contents")
  context {
    let pm = page-map()
    let big = data.recipe_count > 36 and not pp.stacked
    let body = {
      for ch in data.chapters {
        let pg = pm.at(ch.key, default: none)
        block(breakable: false, above: sz(14pt), below: sz(6pt), {
          let title = if th.id == "classic" {
            text(fill: th.accent, tracking: 0.08em, size: sz(11.5pt), smallcaps(lower(ch.title)))
          } else if th.id == "modern" {
            text(font: th.label-font, weight: "semibold", size: sz(9pt), tracking: 0.16em, fill: th.accent, upper(ch.title))
          } else {
            display(ch.title, size: 17pt, weight: "bold", fill: th.accent)
          }
          toc-line(title, if pg != none { text(fill: th.accent, str(pg)) })
        })
        for r in ch.recipes {
          let pg = pm.at(r.key, default: none)
          block(above: sz(4pt), below: sz(4pt), {
            let indent = if th.id == "modern" { 0pt } else { 1em }
            set par(justify: false, hanging-indent: 1em)
            pad(left: indent, toc-line(r.name, if pg != none { str(pg) }))
          })
        }
      }
    }
    if big { columns(2, gutter: 2.2em, body) } else { body }
  }
  mark-end()
}

// ---------------------------------------------------------------------------
// Chapter openers
// ---------------------------------------------------------------------------

#let opener-list(ch) = context {
  let pm = page-map()
  let n = ch.recipes.len()
  if n == 0 or n > 18 { return }
  let row(r) = {
    let pg = pm.at(r.key, default: none)
    if th.id == "modern" {
      block(above: 0.55em, below: 0.55em, par(justify: false, {
        r.name
        if pg != none { h(0.6em); text(fill: th.accent, weight: "semibold", str(pg)) }
      }))
    } else {
      block(above: 0.5em, below: 0.5em, par(justify: false, {
        text(style: "italic", r.name)
        if pg != none {
          text(fill: th.rule, "  ·  ")
          text(fill: th.accent, number-type: "old-style", str(pg))
        }
      }))
    }
  }
  if th.id == "modern" {
    set text(size: sz(10pt))
    if n > 6 and not pp.stacked {
      columns(2, gutter: 2em, ch.recipes.map(row).join())
    } else {
      ch.recipes.map(row).join()
    }
  } else {
    set align(center)
    set text(size: sz(10.5pt))
    ch.recipes.map(row).join()
  }
}

#let chapter-opener(ch, first: false) = {
  pagebreak(to: "odd", weak: true)
  page(header: none, footer: none, {
    if first { counter(page).update(1) }
    anchor(ch.key)
    mark-start(ch.title, folio: false)
    if th.id == "classic" {
      set align(center)
      v(1.1fr)
      if data.grouped {
        text(size: sz(11pt), tracking: 0.3em, fill: th.accent, smallcaps(lower(chapter-number(ch.number))))
        v(sz(18pt))
      }
      heading(level: 1, ch.title)
      v(sz(14pt))
      fleuron-rule(width: 2.4in, glyph: th.fleuron-alt, size: 15pt)
      v(sz(30pt))
      opener-list(ch)
      v(2fr)
    } else if th.id == "modern" {
      v(0.7fr)
      if data.grouped {
        display(chapter-number(ch.number), size: 96pt, fill: th.accent)
        v(sz(-6pt))
      }
      heading(level: 1, ch.title)
      v(sz(10pt))
      box(width: 48pt, height: 4pt, fill: th.accent)
      v(sz(30pt))
      block(width: 100%, opener-list(ch))
      v(1.3fr)
    } else {
      set align(center)
      v(0.9fr)
      block(width: 82%, stroke: 1.2pt + th.rule, inset: 3.5pt,
        block(width: 100%, stroke: 0.45pt + th.rule, inset: (x: 18pt, y: 26pt), {
          text(font: th.ornament-font, size: sz(15pt), fill: th.accent, "☙ " + th.fleuron + " ❧")
          v(sz(6pt))
          if data.grouped {
            text(size: sz(9pt), tracking: 0.28em, fill: th.muted, upper(chapter-number(ch.number)))
            v(sz(4pt))
          }
          heading(level: 1, ch.title)
          v(sz(4pt))
          box(width: 1.6in, double-rule())
          v(sz(16pt))
          opener-list(ch)
        }))
      v(2fr)
    }
  })
}

#show heading.where(level: 1): it => {
  set par(justify: false, leading: 0.35em)
  if th.id == "classic" {
    block(width: 90%, display(it.body, size: 38pt))
  } else if th.id == "modern" {
    block(width: 95%, display(it.body, size: 52pt))
  } else {
    block(width: 100%, display(it.body, size: 58pt, weight: "bold", fill: th.accent))
  }
}

// ---------------------------------------------------------------------------
// Recipes
// ---------------------------------------------------------------------------

#show heading.where(level: 2): it => {
  set par(justify: false, leading: 0.32em)
  if th.id == "classic" {
    align(center, block(width: 92%, display(it.body, size: 25pt)))
  } else if th.id == "modern" {
    block(width: 100%, display(it.body, size: 30pt))
  } else {
    align(center, block(width: 94%, display(it.body, size: 34pt, weight: "bold", fill: th.accent)))
  }
}

#let meta-line(r) = {
  if r.meta.len() == 0 { return }
  if th.id == "modern" {
    block(width: 100%, above: sz(14pt), below: sz(14pt), stroke: (top: 0.6pt + th.rule, bottom: 0.6pt + th.rule), inset: (y: sz(8pt)), {
      set par(justify: false)
      grid(
        columns: r.meta.len() * (auto,),
        column-gutter: sz(26pt),
        ..r.meta.map(m => {
          label-text(m.label, size: 8pt)
          linebreak()
          text(size: sz(10.5pt), weight: "semibold", m.value)
        }),
      )
    })
  } else if th.id == "classic" {
    let items = r.meta.map(m => box[#text(size: sz(10.5pt), fill: th.accent, tracking: 0.08em, smallcaps(lower(m.label)))#h(0.35em)#m.value])
    align(center, block(above: sz(10pt), below: sz(10pt), par(justify: false, items.join(text(fill: th.rule, h(0.7em) + "·" + h(0.7em))))))
  } else {
    let items = r.meta.map(m => box[#text(size: sz(8.5pt), fill: th.accent, weight: "semibold", tracking: 0.12em, upper(m.label))#h(0.4em)#text(style: "italic", m.value)])
    align(center, block(above: sz(10pt), below: sz(10pt), par(justify: false, items.join(text(font: th.ornament-font, fill: th.rule, h(0.6em) + "❦" + h(0.6em))))))
  }
}

#let description(r) = {
  if r.description.len() == 0 { return }
  set par(justify: false, leading: 0.55em)
  if th.id == "modern" {
    block(width: 88%, above: sz(10pt), below: sz(4pt), {
      set text(size: sz(11.5pt), fill: th.muted)
      r.description.join(parbreak())
    })
  } else {
    align(center, block(width: 84%, above: sz(8pt), below: sz(4pt), {
      set text(size: sz(11.5pt), style: "italic", fill: th.muted)
      r.description.join(parbreak())
    }))
  }
}

// A cropped photo of the given size.
#let photo-box(path, w, h) = block(width: w, height: h, clip: true, spacing: 0pt, image(path, width: 100%, height: 100%, fit: "cover"))

// Photo shown under the title (landscape) — or in the side column (portrait).
#let banner-photo(r) = {
  let path = r.image.path
  let h = pp.photo-height
  if r.image.aspect < 1.0 { h = h * 1.12 }
  if th.id == "classic" {
    block(above: sz(10pt), below: sz(16pt), width: 100%,
      align(center, box(stroke: 0.5pt + th.rule, inset: 4pt, photo-box(path, 100% - 8pt, h))))
  } else if th.id == "modern" {
    // Edge-to-edge across the page width.
    block(above: sz(4pt), below: sz(22pt), context {
      let m = pp.margin
      let (lm, rm) = if is-odd-page() { (m.inside, m.outside) } else { (m.outside, m.inside) }
      pad(left: -lm, right: -rm, photo-box(path, 100%, h))
    })
  } else {
    // A snapshot pinned slightly askew, with a soft shadow.
    block(above: sz(14pt), below: sz(22pt), width: 100%, align(center, {
      layout(size => {
        let angle = -1.2deg
        let ph = h * 0.92
        let frame = 14pt + 0.6pt + 4pt // insets + stroke + shadow offset
        let (c, sn) = (calc.cos(angle), calc.abs(calc.sin(angle)))
        let pw = calc.min(4.6in * th.s, (size.width - frame * c - (ph + frame) * sn - 2pt) / c)
        rotate(angle, reflow: true, box({
          place(dx: 3.5pt, dy: 4pt, block(width: 100%, height: 100%, fill: th.rule.transparentize(55%)))
          box(fill: white, stroke: 0.6pt + th.rule, inset: (x: 7pt, top: 7pt, bottom: 7pt), photo-box(path, pw, ph))
        }))
      })
    }))
  }
}

#let side-photo(r) = {
  let path = r.image.path
  if th.id == "rustic" {
    // Size the photo so the *rotated* frame's bounding box fits the column.
    let angle = 1.2deg
    let (c, sn) = (calc.cos(angle), calc.sin(angle))
    let frame = 6pt * 2 + 0.6pt
    block(below: sz(16pt), width: 100%, layout(size => {
      let iw = (size.width - frame * c - frame * sn - 1pt) / (c + sn / r.image.aspect)
      align(center, rotate(angle, reflow: true, box(fill: white, stroke: 0.6pt + th.rule, inset: 6pt,
        image(path, width: iw, height: iw / r.image.aspect))))
    }))
  } else if th.id == "classic" {
    block(below: sz(16pt), box(stroke: 0.5pt + th.rule, inset: 3pt, image(path, width: 100%)))
  } else {
    block(below: sz(16pt), image(path, width: 100%))
  }
}

#let column-heading(title) = {
  if th.id == "classic" {
    block(above: 0pt, below: sz(9pt), {
      text(size: sz(12.5pt), fill: th.accent, tracking: 0.14em, smallcaps(lower(title)))
      v(-0.6em)
      hrule(width: 2.2em, stroke: 0.5pt, fill: th.accent)
    })
  } else if th.id == "modern" {
    block(above: 0pt, below: sz(10pt), label-text(title, size: 9pt))
  } else {
    block(above: 0pt, below: sz(8pt), display(title, size: 21pt, weight: "bold", fill: th.accent))
  }
}

#let sub-heading(title) = {
  if th.id == "classic" {
    block(above: sz(11pt), below: sz(5pt), sticky: true, text(style: "italic", fill: th.accent, title))
  } else if th.id == "modern" {
    block(above: sz(12pt), below: sz(5pt), sticky: true, text(weight: "semibold", fill: th.ink, title))
  } else {
    block(above: sz(11pt), below: sz(5pt), sticky: true, text(style: "italic", weight: "semibold", fill: th.accent, title))
  }
}

#let ingredient-list(r) = {
  set par(justify: false, leading: 0.5em, hanging-indent: 0.9em)
  for g in r.ingredients {
    if g.title != none { sub-heading(g.title) }
    for it in g.items {
      block(above: sz(5.5pt), below: sz(5.5pt), par({
        if it.amount != "" {
          if th.id == "modern" {
            text(weight: "semibold", it.amount)
          } else if th.id == "classic" {
            text(fill: th.accent, it.amount)
          } else {
            text(weight: "semibold", fill: th.accent, it.amount)
          }
          h(0.3em)
        }
        it.item
      }))
    }
  }
}

#let ingredients-block(r, with-photo: false, has-method: true) = {
  if with-photo { side-photo(r) }
  let count = r.ingredients.map(g => g.items.len()).sum(default: 0)
  let body = {
    column-heading("Ingredients")
    if (pp.stacked or not has-method) and count > 6 {
      columns(2, gutter: sz(18pt), ingredient-list(r))
    } else {
      ingredient-list(r)
    }
  }
  if th.id == "modern" {
    block(width: 100%, fill: th.tint, inset: (x: sz(12pt), y: sz(14pt)), radius: 2pt, body)
  } else if th.id == "rustic" {
    block(width: 100%, stroke: 1.1pt + th.rule, inset: 3pt,
      block(width: 100%, stroke: 0.4pt + th.rule, inset: (x: sz(10pt), y: sz(11pt)), body))
  } else {
    body
  }
}

#let step-number(n) = {
  if th.id == "classic" {
    text(fill: th.accent, size: sz(13pt), number-type: "old-style", str(n) + ".")
  } else if th.id == "modern" {
    text(font: th.display-font, fill: th.accent, size: sz(17pt), str(n))
  } else {
    box(baseline: 22%, circle(radius: sz(8pt), stroke: 0.7pt + th.accent, inset: 0pt,
      align(center + horizon, text(font: th.display-font, weight: "bold", size: sz(13pt), fill: th.accent, str(n)))))
  }
}

#let method-block(r, offset: false) = {
  if offset and th.id == "modern" { v(sz(14pt)) }
  if offset and th.id == "rustic" { v(sz(11pt) + 3.5pt) }
  column-heading("Instructions")
  for g in r.steps {
    if g.title != none { sub-heading(g.title) }
    let num-w = if th.id == "modern" { sz(26pt) } else if th.id == "rustic" { sz(24pt) } else { sz(22pt) }
    let dy = if th.id == "modern" { -0.25em } else if th.id == "rustic" { -0.15em } else { -0.12em }
    grid(
      columns: (num-w, 1fr),
      row-gutter: sz(9pt),
      ..g.steps.enumerate().map(((i, s)) => (move(dy: dy, step-number(i + 1)), par(s))).flatten(),
    )
  }
}

#let notes-block(r) = {
  if r.notes.len() == 0 { return }
  let body = r.notes.join(parbreak())
  if th.id == "classic" {
    block(width: 100%, above: sz(20pt), breakable: true, stroke: (top: 0.5pt + th.rule, bottom: 0.5pt + th.rule), inset: (x: 0pt, y: sz(10pt)), {
      align(center, text(size: sz(11pt), fill: th.accent, tracking: 0.16em, [#text(font: th.ornament-font, th.fleuron-alt) #h(0.3em) #smallcaps("notes")]))
      v(sz(2pt))
      set text(style: "italic")
      set par(justify: true)
      body
    })
  } else if th.id == "modern" {
    block(width: 100%, above: sz(22pt), breakable: true, fill: th.tint, stroke: (left: 3pt + th.accent), inset: (x: sz(14pt), y: sz(12pt)), {
      label-text("Notes", size: 9pt)
      v(sz(2pt))
      body
    })
  } else {
    block(width: 100%, above: sz(20pt), breakable: true, stroke: (paint: th.rule, thickness: 0.8pt, dash: "dashed"), radius: 3pt, inset: (x: sz(14pt), y: sz(10pt)), {
      display("Kitchen Notes", size: 19pt, weight: "bold", fill: th.accent)
      v(sz(2pt))
      set text(style: "italic")
      body
    })
  }
}

#let nutrition-block(r) = {
  if r.nutrition.len() == 0 { return }
  block(above: sz(16pt), breakable: false, width: 100%, {
    if th.id == "classic" {
      text(size: sz(10pt), fill: th.accent, tracking: 0.14em, smallcaps("nutrition per serving"))
    } else if th.id == "modern" {
      label-text("Nutrition per serving", size: 8pt)
    } else {
      text(size: sz(8pt), fill: th.accent, weight: "semibold", tracking: 0.14em, upper("Nutrition per serving"))
    }
    v(sz(1pt))
    set text(size: sz(8.5pt))
    let cols = if pp.stacked { 2 } else { 4 }
    table(
      columns: cols * (1fr, auto),
      stroke: (x, y) => (bottom: 0.4pt + th.rule),
      inset: (x: 4pt, y: 4pt),
      align: (x, y) => if calc.odd(x) { right } else { left },
      ..r.nutrition.map(m => (text(fill: th.muted, m.label), m.value)).flatten(),
    )
  })
}

#let source-line(r) = {
  if r.source == none { return }
  let s = r.source
  block(above: sz(16pt), {
    set text(size: sz(8.5pt), fill: th.muted)
    set par(justify: false)
    let al = if th.id == "modern" { left } else { center }
    align(al, {
      if th.id == "classic" { smallcaps("source") } else { text(tracking: 0.12em, weight: "semibold", "SOURCE") }
      h(0.5em)
      if s.name != none { text(style: "italic", s.name) }
      if s.name != none and s.host != none { h(0.25em); "·"; h(0.25em) }
      if s.host != none {
        if s.url != none { link(s.url, s.host) } else { s.host }
      }
    })
  })
}

#let recipe-page(r) = {
  pagebreak(weak: true)
  anchor(r.key)
  let has-img = r.image != none
  let has-ing = r.ingredients.len() > 0
  let has-steps = r.steps.len() > 0
  let portrait = has-img and r.image.aspect < 0.95
  let side = not pp.stacked and has-ing and has-steps

  if th.id == "modern" and has-img and not (portrait and side) {
    banner-photo(r)
  }
  heading(level: 2, r.name)
  if th.id == "classic" {
    align(center, block(above: sz(9pt), below: sz(6pt), text(font: th.ornament-font, size: sz(12pt), fill: th.accent, th.fleuron)))
  } else if th.id == "rustic" {
    align(center, block(above: sz(10pt), below: sz(4pt), box(width: 1.5in, double-rule())))
  }
  description(r)
  meta-line(r)
  if th.id != "modern" and has-img and not (portrait and side) {
    banner-photo(r)
  } else if th.id != "modern" and r.meta.len() == 0 {
    v(sz(10pt))
  }

  if side {
    grid(
      columns: (th.ing-width, 1fr),
      column-gutter: if th.id == "classic" { sz(26pt) } else { sz(24pt) },
      ingredients-block(r, with-photo: portrait),
      method-block(r, offset: not portrait),
    )
  } else {
    if has-ing {
      ingredients-block(r, has-method: has-steps)
      v(sz(14pt))
    }
    if has-steps { method-block(r) }
    if not has-ing and not has-steps and r.notes.len() == 0 {
      align(center, text(style: "italic", fill: th.muted, "No ingredients or instructions recorded for this recipe."))
    }
  }
  notes-block(r)
  nutrition-block(r)
  source-line(r)
}

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

#let index-section() = {
  pagebreak(to: "odd", weak: true)
  anchor("index")
  mark-start("Index")
  section-title("Index")
  context {
    let pm = page-map()
    set text(size: sz(8.6pt))
    set par(justify: false, leading: 0.45em, hanging-indent: 1.4em)
    let pg(key) = {
      let p = pm.at(key, default: none)
      if p == none { "" } else { str(p) }
    }
    columns(pp.columns, gutter: sz(16pt), {
      for g in data.index_groups {
        block(above: sz(12pt), below: sz(5pt), sticky: true, {
          if th.id == "classic" {
            text(size: sz(13pt), fill: th.accent, tracking: 0.1em, g.letter)
          } else if th.id == "modern" {
            display(g.letter, size: 17pt, fill: th.accent)
          } else {
            display(g.letter, size: 20pt, weight: "bold", fill: th.accent)
          }
        })
        for e in g.entries {
          if e.key != none {
            block(above: 0.42em, below: 0.42em, {
              if th.id == "modern" { text(weight: "semibold", e.term) } else { text(style: "italic", e.term) }
              ", "
              text(fill: th.accent, pg(e.key))
            })
          } else {
            block(above: 0.55em, below: 0.3em, sticky: true, text(weight: "semibold", e.term))
            for s in e.subs {
              block(above: 0.3em, below: 0.3em, {
                h(0.9em)
                s.name
                ", "
                text(fill: th.accent, pg(s.key))
              })
            }
          }
        }
      }
    })
  }
  mark-end()
}

// ---------------------------------------------------------------------------
// The book
// ---------------------------------------------------------------------------

#title-page()
#if data.dedication.len() > 0 {
  pagebreak(to: "odd", weak: true)
  dedication-page()
}
#if data.toc { toc-page() }

#if data.chapters.len() == 0 {
  pagebreak(to: "odd", weak: true)
  page(header: none, footer: none, align(center + horizon, {
    text(font: th.ornament-font, size: sz(20pt), fill: th.accent, th.fleuron)
    v(sz(10pt))
    text(style: "italic", size: sz(13pt), fill: th.muted, "No recipes yet — this cookbook is waiting to be filled.")
  }))
}

#for (i, ch) in data.chapters.enumerate() {
  chapter-opener(ch, first: i == 0)
  for r in ch.recipes { recipe-page(r) }
  mark-end()
}

#if data.index and data.index_groups.len() > 0 { index-section() }
