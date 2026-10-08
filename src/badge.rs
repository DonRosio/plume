//! Unread badge. Discord already puts the number of unread mentions at the start of its page
//! title ("(3) Discord | #général"): Plume only reads that title — nothing is injected into
//! the page — and draws a small red badge with the number.

/// Number of unread mentions announced by the page title, 0 if none.
pub fn mentions_from_title(title: &str) -> u32 {
    let Some(rest) = title.trim_start().strip_prefix('(') else {
        return 0;
    };
    let Some((number, _)) = rest.split_once(')') else {
        return 0;
    };
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    // "(1 234)" or "(99+)" style formats are tolerated; anything else is ignored.
    if digits.is_empty()
        || !number
            .chars()
            .all(|c| c.is_ascii_digit() || c == '+' || c.is_whitespace())
    {
        return 0;
    }
    digits.parse().unwrap_or(u32::MAX)
}

/// Text shown in the badge: "1" to "9", then "9+".
pub fn label(count: u32) -> &'static str {
    const LABELS: [&str; 10] = ["", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    match count {
        0..=9 => LABELS[count as usize],
        _ => "9+",
    }
}

/// 3×5 pixel font for the badge glyphs.
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        _ => [0; 5],
    }
}

const RED: [u8; 4] = [237, 66, 69, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

/// Draws a red disc with the label, centred, as `size`×`size` RGBA pixels starting at
/// (`left`, `top`) inside an image `stride` pixels wide.
fn draw(rgba: &mut [u8], stride: u32, left: u32, top: u32, size: u32, count: u32) {
    let text = label(count);
    let scale = (size / 8).max(1);
    let text_w = (text.len() as u32 * 4 - 1) * scale;
    let text_h = 5 * scale;
    let (tx, ty) = (
        (size.saturating_sub(text_w)) / 2,
        (size.saturating_sub(text_h)) / 2,
    );
    let radius = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - radius, y as f32 + 0.5 - radius);
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let mut color = RED;
            if x >= tx && y >= ty && x < tx + text_w && y < ty + text_h {
                let (gx, gy) = ((x - tx) / scale, (y - ty) / scale);
                let (index, column) = ((gx / 4) as usize, gx % 4);
                if column < 3 {
                    let ch = text.chars().nth(index).unwrap_or(' ');
                    if glyph(ch)[gy as usize] & (0b100 >> column) != 0 {
                        color = WHITE;
                    }
                }
            }
            let offset = (((top + y) * stride + left + x) * 4) as usize;
            if let Some(pixel) = rgba.get_mut(offset..offset + 4) {
                pixel.copy_from_slice(&color);
            }
        }
    }
}

/// Stand-alone badge (taskbar overlay icon), `size`×`size` RGBA.
pub fn overlay(count: u32, size: u32) -> Vec<u8> {
    let mut rgba = vec![0; (size * size * 4) as usize];
    draw(&mut rgba, size, 0, 0, size, count);
    rgba
}

/// The application icon with the badge in its bottom-right corner (notification area).
pub fn on_icon(icon: &[u8], icon_size: u32, count: u32) -> Vec<u8> {
    let mut rgba = icon.to_vec();
    let size = icon_size * 5 / 8;
    draw(
        &mut rgba,
        icon_size,
        icon_size - size,
        icon_size - size,
        size,
        count,
    );
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_discord_titles() {
        assert_eq!(
            mentions_from_title("(3) Discord | #général | Rust France"),
            3
        );
        assert_eq!(mentions_from_title("(12) Discord"), 12);
        assert_eq!(mentions_from_title("(99+) Discord"), 99);
        assert_eq!(mentions_from_title("Discord | Amis"), 0);
        assert_eq!(mentions_from_title("• Discord | #général"), 0);
        assert_eq!(mentions_from_title("(abc) Discord"), 0);
        assert_eq!(mentions_from_title("(99999999999999) Discord"), u32::MAX);
        assert_eq!(mentions_from_title(""), 0);
    }

    #[test]
    fn labels() {
        assert_eq!(label(0), "");
        assert_eq!(label(7), "7");
        assert_eq!(label(10), "9+");
        assert_eq!(label(u32::MAX), "9+");
    }

    #[test]
    fn drawing_stays_in_bounds() {
        let badge = overlay(5, 16);
        assert_eq!(badge.len(), 16 * 16 * 4);
        assert_eq!(&badge[0..4], &[0, 0, 0, 0], "corners stay transparent");
        let centre = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(badge[centre + 3], 255);
        let icon = vec![9u8; 64 * 64 * 4];
        let with_badge = on_icon(&icon, 64, 42);
        assert_eq!(with_badge.len(), icon.len());
        assert_eq!(
            &with_badge[0..4],
            &[9, 9, 9, 9],
            "top-left of the icon untouched"
        );
    }
}
