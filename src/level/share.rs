//! Share links: a whole level file inside a link to the web game,
//! `https://rabbiveesh.github.io/nat-han-adventures/#lvl=1<code>`, where the code is the file
//! deflated and written in URL-safe base64 (`1` is the format's version). It sits after the
//! `#`, so it never leaves the player's browser: no server, no accounts. The level editor makes
//! them (Share), the game plays them (`game::playtest`). A link's level came from a stranger:
//! [`decode`] caps its size and the parser rejects what doesn't parse.

/// The web game.
pub const GAME_URL: &str = "https://rabbiveesh.github.io/nat-han-adventures/";
/// What starts a level code in a link's `#...`.
pub const KEY: &str = "lvl=";
/// The largest level file a code may hold (the biggest story level is about 8 KB).
pub const MAX_TEXT: usize = 64 * 1024;
/// The largest code accepted (a full `MAX_TEXT` of noise, deflated, is a bit over this).
pub const MAX_CODE: usize = 96 * 1024;
const VERSION: char = '1';
const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// A level file's code.
pub fn encode(text: &str) -> String {
    let packed = miniz_oxide::deflate::compress_to_vec(text.as_bytes(), 10);
    let mut out = String::with_capacity(1 + packed.len() * 4 / 3 + 3);
    out.push(VERSION);
    for chunk in packed.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// A share link for a level file.
pub fn link(text: &str) -> String {
    format!("{GAME_URL}#{KEY}{}", encode(text))
}

/// The level file in a share link, the `#...` part of one, or a bare code.
pub fn decode(link_or_code: &str) -> Result<String, String> {
    let s = link_or_code.trim();
    let code = s.rsplit_once(KEY).map_or(s, |(_, c)| c);
    // Chat apps sometimes tack on punctuation or break the line.
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let code = code.trim_end_matches(['.', ',', ')', '>', '"', '\'']);
    if code.len() > MAX_CODE {
        return Err("that level code is too long".into());
    }
    let Some(body) = code.strip_prefix(VERSION) else {
        return Err(if code.is_empty() { "no level code there".into() } else { "not a level code (or from a newer game)".into() });
    };
    let mut bytes = Vec::with_capacity(body.len() * 3 / 4);
    let (mut n, mut bits) = (0u32, 0u32);
    for c in body.bytes() {
        let v = B64.iter().position(|&b| b == c).ok_or("that level code is broken (a stray character)")? as u32;
        n = n << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((n >> bits) as u8);
            n &= (1 << bits) - 1;
        }
    }
    let text = miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes, MAX_TEXT)
        .map_err(|e| match e.status {
            miniz_oxide::inflate::TINFLStatus::HasMoreOutput => "that level is too big".to_string(),
            _ => "that level code is broken (cut short?)".to_string(),
        })?;
    String::from_utf8(text).map_err(|_| "that level code is broken".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_survives_a_link_and_fits_one() {
        let all = crate::level::LEVEL_SOURCES.iter().chain(crate::game::sideb::SIDE_B_SOURCES.iter());
        for text in all {
            let l = link(text);
            assert!(l.len() < 2000, "{} chars", l.len());
            assert_eq!(decode(&l).as_deref(), Ok(*text));
            assert_eq!(decode(&encode(text)).as_deref(), Ok(*text), "a bare code");
            assert_eq!(decode(&format!("  {l}.\n")).as_deref(), Ok(*text), "pasted out of a chat");
        }
        for text in ["", "a", "ab", "abc", "abcd"] {
            assert_eq!(decode(&encode(text)).as_deref(), Ok(text));
        }
    }

    #[test]
    fn junk_is_refused_not_a_crash() {
        let good = encode(crate::level::LEVEL_SOURCES[0]);
        for bad in ["", "#lvl=", "hello", "2abc", "1***", &good[..good.len() / 2]] {
            assert!(decode(bad).is_err(), "{bad:?}");
        }
        // A bomb: tiny code, huge level.
        assert_eq!(decode(&encode(&"#".repeat(MAX_TEXT + 1))), Err("that level is too big".into()));
        assert!(decode(&encode(&"#".repeat(MAX_TEXT))).is_ok());
    }
}
