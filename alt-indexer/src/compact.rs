use crate::card::{CardEffect, CardJson};
use crate::path::ParsedCardPath;
use anyhow::{Context, Result};
use std::path::Path;

/// Fixed-size compact representation of a card, matching the 32-byte record layout.
#[derive(Debug, Clone)]
pub struct CompactCardFields {
    pub faction_code: u8,           // 0..=6
    pub main_cost: u8,              // 0..=15
    pub recall_cost: u8,            // 0..=15
    pub mountain_power: u8,         // 0..=15
    pub ocean_power: u8,            // 0..=15
    pub forest_power: u8,           // 0..=15
    pub main_effect: [[u16; 3]; 3], // [group][T,C,O], 0..=4095
    pub echo_effect: [u16; 3],      // [T,C,O], 0..=4095
}

pub const RECORD_SIZE: usize = 32;

/// Map `mainFaction.reference` to the on-disk faction code (`0` = unknown).
pub fn faction_code_from_reference(reference: &str) -> u8 {
    match reference {
        "AX" => 1,
        "BR" => 2,
        "LY" => 3,
        "MU" => 4,
        "OR" => 5,
        "YZ" => 6,
        _ => 0,
    }
}

/// Extract compact fields from an already-parsed card.
pub fn compact_fields_from_card(card: &CardJson) -> CompactCardFields {
    // Faction: mainFaction.reference only (not path faction).
    let faction_code = card
        .main_faction
        .as_ref()
        .and_then(|mf| mf.reference.as_deref())
        .map(faction_code_from_reference)
        .unwrap_or(0);

    let mut main_cost: u8 = 0;
    let mut recall_cost: u8 = 0;
    let mut mountain_power: u8 = 0;
    let mut ocean_power: u8 = 0;
    let mut forest_power: u8 = 0;

    let mut main_effect: [[u16; 3]; 3] = [[0; 3]; 3];
    let mut echo_effect: [u16; 3] = [0; 3];

    for element in &card.card_elements {
        let elem_type = element
            .card_element_type
            .as_ref()
            .and_then(|t| t.reference.as_ref())
            .map(String::as_str);

        match elem_type {
            Some("MAIN_COST") => {
                if let Some(v) = &element.value {
                    if let Ok(n) = v.parse::<u8>() {
                        main_cost = n;
                    }
                }
            }
            Some("RECALL_COST") => {
                if let Some(v) = &element.value {
                    if let Ok(n) = v.parse::<u8>() {
                        recall_cost = n;
                    }
                }
            }
            Some("MOUNTAIN_POWER") => {
                if let Some(v) = &element.value {
                    if let Ok(n) = v.parse::<u8>() {
                        mountain_power = n;
                    }
                }
            }
            Some("OCEAN_POWER") => {
                if let Some(v) = &element.value {
                    if let Ok(n) = v.parse::<u8>() {
                        ocean_power = n;
                    }
                }
            }
            Some("FOREST_POWER") => {
                if let Some(v) = &element.value {
                    if let Ok(n) = v.parse::<u8>() {
                        forest_power = n;
                    }
                }
            }
            Some("MAIN_EFFECT") => {
                for (group_idx, display) in element.card_effect_displays.iter().take(3).enumerate()
                {
                    if let Some(effect) = &display.card_effect {
                        set_group_from_effect(&mut main_effect[group_idx], effect);
                    }
                }
            }
            Some("ECHO_EFFECT") => {
                let mut trigger: u16 = 0;
                let mut condition: u16 = 0;
                let mut output: u16 = 0;

                for display in &element.card_effect_displays {
                    if let Some(effect) = &display.card_effect {
                        for node in &effect.card_effect_elements {
                            let id = node.id_gd as u16;
                            let kind = node.element_type.as_deref().unwrap_or_default();
                            match kind {
                                "TRIGGER" if trigger == 0 => trigger = id,
                                "CONDITION" if condition == 0 => condition = id,
                                "OUTPUT" if output == 0 => output = id,
                                _ => {}
                            }
                        }
                    }
                }

                echo_effect = [trigger, condition, output];
            }
            _ => {}
        }
    }

    CompactCardFields {
        faction_code,
        main_cost,
        recall_cost,
        mountain_power,
        ocean_power,
        forest_power,
        main_effect,
        echo_effect,
    }
}

/// Read, parse, and extract compact fields (convenience for callers that only need compact data).
pub fn extract_compact_fields(path: &Path, parsed_path: &ParsedCardPath) -> Result<CompactCardFields> {
    let _ = parsed_path;
    let card = crate::card::load_card(path, None)?;
    Ok(compact_fields_from_card(&card))
}

fn set_group_from_effect(slot: &mut [u16; 3], effect: &CardEffect) {
    let mut trigger: u16 = 0;
    let mut condition: u16 = 0;
    let mut output: u16 = 0;

    for node in &effect.card_effect_elements {
        let id = node.id_gd as u16;
        let kind = node.element_type.as_deref().unwrap_or_default();
        match kind {
            "TRIGGER" if trigger == 0 => trigger = id,
            "CONDITION" if condition == 0 => condition = id,
            "OUTPUT" if output == 0 => output = id,
            _ => {}
        }
    }

    slot[0] = trigger;
    slot[1] = condition;
    slot[2] = output;
}

// --- Binary encoding / writing ---

pub fn encode_record(fields: &CompactCardFields) -> [u8; RECORD_SIZE] {
    let mut buf = [0u8; RECORD_SIZE];

    buf[0] = fields.faction_code;
    buf[1] = fields.main_cost;
    buf[2] = fields.recall_cost;
    buf[3] = fields.mountain_power;
    buf[4] = fields.ocean_power;
    buf[5] = fields.forest_power;

    let ids: [u16; 12] = [
        fields.main_effect[0][0], fields.main_effect[0][1], fields.main_effect[0][2],
        fields.main_effect[1][0], fields.main_effect[1][1], fields.main_effect[1][2],
        fields.main_effect[2][0], fields.main_effect[2][1], fields.main_effect[2][2],
        fields.echo_effect[0], fields.echo_effect[1], fields.echo_effect[2],
    ];

    let mut offset = 6;
    for id in ids {
        let bytes = id.to_le_bytes();
        buf[offset] = bytes[0];
        buf[offset + 1] = bytes[1];
        offset += 2;
    }

    buf
}

pub fn write_compact_records(
    path: &Path,
    total_bit_span: u32,
    cards: &[(u32, CompactCardFields)],
) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;

    file.set_len((total_bit_span as u64) * RECORD_SIZE as u64)
        .with_context(|| format!("set_len {}", path.display()))?;

    for (card_index, fields) in cards {
        let offset = (*card_index as u64) * RECORD_SIZE as u64;
        file.seek(SeekFrom::Start(offset))
            .with_context(|| format!("seek {} to {}", path.display(), offset))?;
        file.write_all(&encode_record(fields))
            .with_context(|| format!("write record at {} in {}", offset, path.display()))?;
    }

    Ok(())
}

// --- Read-side view ---

pub struct CompactCardView<'a> {
    buf: &'a [u8; RECORD_SIZE],
}

impl<'a> CompactCardView<'a> {
    pub fn from_data(data: &'a [u8], card_index: u32) -> Option<Self> {
        let offset = card_index as usize * RECORD_SIZE;
        let slice = data.get(offset..offset + RECORD_SIZE)?;
        let buf: &[u8; RECORD_SIZE] = slice.try_into().ok()?;
        Some(Self { buf })
    }

    pub fn faction_code(&self) -> u8 {
        self.buf[0]
    }
    pub fn main_cost(&self) -> u8 {
        self.buf[1]
    }
    pub fn recall_cost(&self) -> u8 {
        self.buf[2]
    }
    pub fn mountain_power(&self) -> u8 {
        self.buf[3]
    }
    pub fn ocean_power(&self) -> u8 {
        self.buf[4]
    }
    pub fn forest_power(&self) -> u8 {
        self.buf[5]
    }

    pub fn id_gd(&self, idx: usize) -> u16 {
        let base = 6 + idx * 2;
        u16::from_le_bytes([self.buf[base], self.buf[base + 1]])
    }

    pub fn main_effect_group(&self, group: usize) -> [u16; 3] {
        let base = group * 3;
        [self.id_gd(base), self.id_gd(base + 1), self.id_gd(base + 2)]
    }

    pub fn echo_effect(&self) -> [u16; 3] {
        [self.id_gd(9), self.id_gd(10), self.id_gd(11)]
    }
}
