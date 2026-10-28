use nostalre_renderer::font_atlas::FontAtlas;
use std::borrow::Cow;

/// `text` cut to `max_w` pixels, ending in `…`, when it does not fit; as it is when it does.
pub fn truncate_to_width<'a>(text: &'a str, max_w: f32, atlas: &FontAtlas) -> Cow<'a, str> {
    if atlas.measure_text(text) <= max_w {
        return Cow::Borrowed(text);
    }
    let room = max_w - atlas.measure_text("…");
    let mut width = 0.0;
    let end = text
        .char_indices()
        .find_map(|(i, ch)| {
            width += atlas.glyph(ch).advance;
            (width > room).then_some(i)
        })
        .unwrap_or(text.len());
    Cow::Owned(format!("{}…", &text[..end]))
}

/// Group an integer with comma thousands separators, e.g. `28000000` → `"28,000,000"`.
pub fn format_thousands(n: i64) -> String {
    let neg = n < 0;
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    let len = digits.len();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if neg { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_text_is_cut_to_the_width_with_an_ellipsis() {
        let atlas = FontAtlas::from_embedded(20.0, 1.0);
        assert!(matches!(
            truncate_to_width("Prober", 1000.0, &atlas),
            Cow::Borrowed("Prober")
        ));
        let width = atlas.measure_text("SpikeHu…");
        let cut = truncate_to_width("SpikeHuman", width, &atlas);
        assert_eq!(cut, "SpikeHu…");
        assert!(atlas.measure_text(&cut) <= width);
        assert_eq!(truncate_to_width("Lv", 0.0, &atlas), "…");
    }

    #[test]
    fn groups_by_threes() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(1_000), "1,000");
        assert_eq!(format_thousands(28_000_000), "28,000,000");
        assert_eq!(format_thousands(-12_345), "-12,345");
    }
}
