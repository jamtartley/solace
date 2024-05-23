use crossterm::style::Color;

pub(crate) fn hex_to_rgb(s: &str) -> Color {
    let hex = s.trim_start_matches('#');

    let hex = match hex.len() {
        3 => hex
            .chars()
            .map(|c| c.to_string().repeat(2))
            .collect::<String>(),
        6 => hex.to_owned(),
        _ => "FF5722".to_owned(),
    };

    let rgb = u32::from_str_radix(&hex, 16)
        .expect("ERROR: Couldn't convert color from hex")
        .to_be_bytes();

    Color::Rgb {
        r: rgb[1],
        g: rgb[2],
        b: rgb[3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::style::Color;

    #[test]
    fn test_hex_to_rgb_valid_6_digit() {
        assert_eq!(
            hex_to_rgb("#FF5733"),
            Color::Rgb {
                r: 255,
                g: 87,
                b: 51
            }
        );
    }

    #[test]
    fn test_hex_to_rgb_valid_3_digit() {
        assert_eq!(
            hex_to_rgb("#F53"),
            Color::Rgb {
                r: 255,
                g: 85,
                b: 51
            }
        );
    }

    #[test]
    fn test_hex_to_rgb_no_hash() {
        assert_eq!(
            hex_to_rgb("FF5733"),
            Color::Rgb {
                r: 255,
                g: 87,
                b: 51
            }
        );
    }

    #[test]
    fn test_hex_to_rgb_invalid_length() {
        assert_eq!(
            hex_to_rgb("1234"),
            Color::Rgb {
                r: 255,
                g: 87,
                b: 34
            } // Fallback to default #FF5722
        );
    }

    #[test]
    fn test_hex_to_rgb_default_color() {
        assert_eq!(
            hex_to_rgb("invalid"),
            Color::Rgb {
                r: 255,
                g: 87,
                b: 34
            } // Fallback to default #FF5722
        );
    }
}
