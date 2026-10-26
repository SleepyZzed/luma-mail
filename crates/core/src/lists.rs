//! Shared paragraph markers for the editor and its HTML representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bullet,
    Numbered,
}
impl Kind {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Bullet => "ul",
            Self::Numbered => "ol",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marker {
    pub depth: usize,
    pub chars: i32,
    pub kind: Kind,
    pub number: u32,
}
pub fn marker(line: &str) -> Option<Marker> {
    let spaces = line.bytes().take_while(|c| *c == b' ').count();
    let rest = &line[spaces..];
    if rest.starts_with("• ") {
        return Some(Marker {
            depth: (spaces / 2).min(6),
            chars: (spaces + 2) as i32,
            kind: Kind::Bullet,
            number: 0,
        });
    }
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 6 || !rest[digits..].starts_with(". ") {
        return None;
    }
    let number = rest[..digits].parse::<u32>().ok().filter(|n| *n > 0)?;
    Some(Marker {
        depth: (spaces / 2).min(6),
        chars: (spaces + digits + 2) as i32,
        kind: Kind::Numbered,
        number,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_only_paragraph_starts_with_bounded_numbering() {
        assert_eq!(
            marker("  12. Item").unwrap(),
            Marker {
                depth: 1,
                chars: 6,
                kind: Kind::Numbered,
                number: 12
            }
        );
        assert_eq!(marker("• item").unwrap().kind, Kind::Bullet);
        for line in ["a 1. item", "1.item", "0. item", "1234567. item"] {
            assert!(marker(line).is_none());
        }
    }
}
