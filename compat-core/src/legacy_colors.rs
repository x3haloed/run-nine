//! Documented eight-color QuickDraw names and their native historical RGB values.
//! These are distinct from pure additive primary/secondary colors.
pub fn rgb(color: u32) -> Option<[u16; 3]> {
    Some(match color {
        30 => [65535; 3],
        33 => [0; 3],
        69 => [64512, 62333, 1327],
        137 => [62167, 2134, 34028],
        205 => [56683, 2242, 1698],
        273 => [577, 43860, 60159],
        341 => [0, 32768, 4528],
        409 => [0, 0, 54272],
        _ => return None,
    })
}
