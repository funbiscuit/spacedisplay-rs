use byte_unit::Byte;
use tui::symbols;

pub fn byte_to_str(b: Byte, padding: usize) -> String {
    format!(
        "{0}{1}{0}",
        " ".repeat(padding),
        b.get_appropriate_unit(true)
    )
}

pub fn get_unicode_block<'a>(frac: f64) -> &'a str {
    match (frac * 8.0).round() as u16 {
        1 => symbols::block::ONE_EIGHTH,
        2 => symbols::block::ONE_QUARTER,
        3 => symbols::block::THREE_EIGHTHS,
        4 => symbols::block::HALF,
        5 => symbols::block::FIVE_EIGHTHS,
        6 => symbols::block::THREE_QUARTERS,
        7 => symbols::block::SEVEN_EIGHTHS,
        8 => symbols::block::FULL,
        _ => " ",
    }
}
