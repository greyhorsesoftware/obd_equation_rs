/// Maps a byte index to its equation variable name.
///
/// `A`–`Z` for bytes 0–25, then `A1`–`Z1` for 26–51, `A2`–`Z2` for 52–77, and so on
/// (group = `index / 26`, letter = `index % 26`). Shared by the native evaluator's
/// byte binding and the equation generator so both agree on names for bytes past `Z`.
pub fn byte_var_name(index: usize) -> String {
    let letter = (b'A' + (index % 26) as u8) as char;
    let group = index / 26;
    if group == 0 {
        letter.to_string()
    } else {
        format!("{}{}", letter, group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_var_name_convention() {
        assert_eq!(byte_var_name(0), "A");
        assert_eq!(byte_var_name(25), "Z");
        assert_eq!(byte_var_name(26), "A1");
        assert_eq!(byte_var_name(39), "N1"); // old cap
        assert_eq!(byte_var_name(40), "O1"); // used to be unassigned
        assert_eq!(byte_var_name(41), "P1"); // 0181 max
        assert_eq!(byte_var_name(51), "Z1");
        assert_eq!(byte_var_name(52), "A2");
    }
}
