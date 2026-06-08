use index_core::compact::CompactCardView;

/// Bit flags for which effect lines a card uses.
pub const SHAPE_M1: u8 = 0b0001;
pub const SHAPE_M2: u8 = 0b0010;
pub const SHAPE_M3: u8 = 0b0100;
pub const SHAPE_EC: u8 = 0b1000;

/// True when any slot in `group` is non-zero (the line is "present").
fn group_present(group: [u16; 3]) -> bool {
    group.iter().any(|v| *v != 0)
}

/// 4-bit shape from a compact record: bit 0=m1, 1=m2, 2=m3, 3=ec.
pub fn shape_of(view: &CompactCardView<'_>) -> u8 {
    let mut shape = 0u8;
    if group_present(view.main_effect_group(0)) {
        shape |= SHAPE_M1;
    }
    if group_present(view.main_effect_group(1)) {
        shape |= SHAPE_M2;
    }
    if group_present(view.main_effect_group(2)) {
        shape |= SHAPE_M3;
    }
    if group_present(view.echo_effect()) {
        shape |= SHAPE_EC;
    }
    shape
}

/// Human-readable shape label, e.g. `m1`, `m1+m2+ec`, or `empty` when no lines are present.
pub fn shape_label(shape: u8) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(4);
    if shape & SHAPE_M1 != 0 {
        parts.push("m1");
    }
    if shape & SHAPE_M2 != 0 {
        parts.push("m2");
    }
    if shape & SHAPE_M3 != 0 {
        parts.push("m3");
    }
    if shape & SHAPE_EC != 0 {
        parts.push("ec");
    }
    if parts.is_empty() {
        "empty".to_string()
    } else {
        parts.join("+")
    }
}

/// 12-slot strict tuple: `[m1.T, m1.C, m1.O, m2.T, m2.C, m2.O, m3.T, m3.C, m3.O, ec.T, ec.C, ec.O]`.
pub fn strict_tuple(view: &CompactCardView<'_>) -> [u16; 12] {
    let m1 = view.main_effect_group(0);
    let m2 = view.main_effect_group(1);
    let m3 = view.main_effect_group(2);
    let ec = view.echo_effect();
    [
        m1[0], m1[1], m1[2], m2[0], m2[1], m2[2], m3[0], m3[1], m3[2], ec[0], ec[1], ec[2],
    ]
}

/// Compact key for the strict tuple, suitable for use as a stable JSON id.
pub fn strict_tuple_id(tuple: &[u16; 12]) -> String {
    let mut s = String::with_capacity(64);
    for (i, v) in tuple.iter().enumerate() {
        if i == 3 || i == 6 || i == 9 {
            s.push('|');
        } else if i != 0 {
            s.push(',');
        }
        s.push_str(&v.to_string());
    }
    s
}

/// Sorted-dedup set of non-zero idGds across all 12 slots (the canonical "combo" definition).
pub fn id_gd_set(view: &CompactCardView<'_>) -> Vec<u16> {
    let mut ids: Vec<u16> = (0..12)
        .map(|i| view.id_gd(i))
        .filter(|v| *v != 0)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Comma-joined set encoding, e.g. `24,182,191,192,379,779` (empty for no-effect cards).
pub fn id_gd_set_id(set: &[u16]) -> String {
    let mut s = String::with_capacity(set.len() * 4);
    for (i, v) in set.iter().enumerate() {
        if i != 0 {
            s.push(',');
        }
        s.push_str(&v.to_string());
    }
    s
}
