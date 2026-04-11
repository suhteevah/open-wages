//! Parser for MSSN*.DAT mission definition files.
//!
//! These are plaintext files with Windows line endings (`\r\n`), terminated by `~`.
//! Each file defines a complete mission: animation sprites, contract terms,
//! negotiation parameters, prestige, intelligence, enemy roster, equipment,
//! weather, travel options, and special events.

use std::path::Path;

use tracing::{debug, info, trace, warn};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur while parsing a mission file.
#[derive(Debug, thiserror::Error)]
pub enum MissionError {
    #[error("I/O error reading mission file: {0}")]
    Io(#[from] std::io::Error),

    #[error("missing required section: {section}")]
    MissingSection { section: String },

    #[error("parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("unexpected end of file while parsing section: {section}")]
    UnexpectedEof { section: String },

    #[error("expected {expected} enemy rows but found {found}")]
    EnemyRowCount { expected: usize, found: usize },
}

// ---------------------------------------------------------------------------
// Data structs
// ---------------------------------------------------------------------------

/// Sprite/animation corpus file references for a mission.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimationFiles {
    /// Player merc sprites (e.g. `jungsld.cor`)
    pub good_guys: String,
    /// Enemy combatant sprites (e.g. `jungemy.cor`)
    pub bad_guys: String,
    /// Guard dog sprites, or `None` if `null`
    pub dogs: Option<String>,
    /// First NPC type
    pub npc1: Option<String>,
    /// Second NPC type
    pub npc2: Option<String>,
    /// Third NPC or vehicle #1
    pub npc3_vhc1: Option<String>,
    /// Fourth NPC or vehicle #2
    pub npc4_vhc2: Option<String>,
}

/// Contract terms offered by the mission client.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContractTerms {
    /// Day-of-year when contract is offered (1-365)
    pub date_day: u16,
    /// Year when contract is offered
    pub date_year: u16,
    /// Client name / organisation
    pub from: String,
    /// Mission objective description
    pub terms: String,
    /// Bonus condition description
    pub bonus_text: String,
    /// Cash advance paid on accepting the contract (dollars)
    pub advance: u32,
    /// Bonus paid on mission success (dollars)
    pub bonus: u32,
    /// Day-of-year deadline
    pub deadline_day: u16,
    /// Year of deadline
    pub deadline_year: u16,
}

/// Contract negotiation parameters (player counter-offer ladder + AI counter-response).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Negotiation {
    /// Four escalating advance counter-offer amounts
    pub advance: [u32; 4],
    /// Four escalating bonus counter-offer amounts
    pub bonus: [u32; 4],
    /// Four escalating deadline counter-offer days
    pub deadline: [u16; 4],
    /// Four acceptance probabilities (percent, descending)
    pub chance: [u8; 4],
    /// AI counter-response value row (4 dollar amounts + 4 day amounts)
    pub counter_values: [u32; 8],
    /// AI advance probability/weight row (8 values)
    pub counter_advance: [u8; 8],
    /// AI bonus probability/weight row (8 values)
    pub counter_bonus: [u8; 8],
    /// AI deadline probability/weight row (8 values)
    pub counter_deadline: [u8; 8],
}

/// Prestige modifiers for mission outcomes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrestigeConfig {
    /// Mission category (1=rescue, 2=retrieval, 3=special)
    pub mission_type: u8,
    /// Map entrance point index
    pub entrance: u8,
    /// Number of map segments
    pub num_maps: u8,
    /// Prestige gained on mission success
    pub success1: i16,
    /// Secondary success modifier (always 0 in observed data)
    pub success2: i16,
    /// Prestige penalty per wounded-in-action
    pub wia: i8,
    /// Prestige penalty per missing-in-action
    pub mia: i8,
    /// Prestige penalty per killed-in-action
    pub kia: i8,
}

/// A tier of purchasable intelligence with name and cost.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntelTier {
    /// Intelligence provider name
    pub name: String,
    /// Base cost
    pub cost: u32,
    /// Per-item cost
    pub per_item: u32,
}

/// Intelligence purchasing options and summary data.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntelligenceConfig {
    /// Three tiers of intel providers
    pub tiers: [IntelTier; 3],
    /// Enemy headcount (approximate)
    pub men: u8,
    /// Enemy experience level indicator
    pub exp: u8,
    /// Enemy firepower rating
    pub fire_power: u8,
    /// Estimated success chance (percent)
    pub success: u8,
    /// Expected casualty level
    pub casualties: u8,
    /// Terrain/biome type (0=desert, 1=jungle)
    pub scene_type: u8,
    /// Number of intel attachment pages
    pub attachments: u8,
}

/// A single enemy/NPC entry from the ratings chart.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnemyRating {
    /// Overall enemy rating / level
    pub rating: u8,
    /// Damage Power Rating
    pub dpr: u8,
    /// Experience points awarded on kill
    pub exp: u8,
    /// Strength
    pub str_: u8,
    /// Agility
    pub agl: u8,
    /// Willpower
    pub wil: u8,
    /// Weapon Skill
    pub wsk: u8,
    /// Hand-to-Hand Combat skill
    pub hhc: u8,
    /// Technical skill
    pub tch: u8,
    /// Encumbrance capacity
    pub enc: u16,
    /// Action Points per turn
    pub aps: u8,
    /// Spawn probability (percent)
    pub presence_chance: u8,
    /// Unit type (2=enemy, 3=NPC, 4=NPC variant, 7=vehicle/dog)
    pub enemy_type: u8,
}

/// A single enemy/NPC weapon loadout entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnemyWeapon {
    /// Primary weapon index (-1 = none)
    pub weapon1: i8,
    /// Secondary weapon index (-1 = none)
    pub weapon2: i8,
    /// Ammo magazine count for weapon 1
    pub ammo1: u8,
    /// Ammo magazine count for weapon 2
    pub ammo2: u8,
    /// Tertiary item index (-1 = none)
    pub weapon3: i8,
    /// Unlabeled 6th column (equipment/item index, -1 = none)
    pub extra: i8,
}

/// Weather probability weights (must sum to 100).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WeatherTable {
    pub clear: u8,
    pub foggy: u8,
    pub overcast: u8,
    pub light_rain: u8,
    pub heavy_rain: u8,
    pub storm: u8,
}

/// Travel options (3 tiers: cheap/medium/premium).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TravelTable {
    pub cost1: u32,
    pub cost2: u32,
    pub cost3: u32,
    pub days1: u8,
    pub days2: u8,
    pub days3: u8,
}

/// Pre-loaded or recommended equipment counts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EquipmentCounts {
    pub weapons: u8,
    pub ammo: u8,
    pub equipment: u8,
}

/// Recommended equipment item (only present when equipment count > 0).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecommendedItem {
    pub item_id: u16,
    pub count: u16,
}

/// Special event configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpecialConfig {
    /// Number of turns for a special action (0 = none)
    pub turns: u8,
    /// Special event type (0 = none)
    pub special_type: u8,
    /// Item ID involved in special event (0 = none)
    pub item: u8,
    /// Environmental damage type (0 = none)
    pub damage: u8,
    /// Damage message with `%s` placeholder for merc name (if damage > 0)
    pub damage_message: Option<String>,
}

/// A fully parsed mission definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Mission {
    pub animation_files: AnimationFiles,
    pub contract: ContractTerms,
    pub negotiation: Negotiation,
    pub prestige: PrestigeConfig,
    pub intelligence: IntelligenceConfig,
    /// Number of hostile combatant entries
    pub enemy_count: u16,
    /// Number of NPC entries appended after combatants
    pub npc_count: u16,
    /// Enemy/NPC ratings (enemy_count + npc_count rows)
    pub enemy_ratings: Vec<EnemyRating>,
    /// Enemy/NPC weapon loadouts (same count/order as ratings)
    pub enemy_weapons: Vec<EnemyWeapon>,
    pub preloaded_equipment: EquipmentCounts,
    pub recommended_equipment: EquipmentCounts,
    /// Recommended equipment item, if any
    pub recommended_item: Option<RecommendedItem>,
    /// Mission start hour (0-23)
    pub start_hour: u8,
    /// Mission start minute (0-59)
    pub start_minute: u8,
    pub weather: WeatherTable,
    pub travel: TravelTable,
    pub special: SpecialConfig,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a MSSN*.DAT mission definition file.
///
/// Reads the file, strips `\r`, and processes sections by label.
/// Returns a fully populated [`Mission`] or a [`MissionError`].
pub fn parse_mission(path: &Path) -> Result<Mission, MissionError> {
    info!(path = %path.display(), "Parsing mission file");

    let raw = std::fs::read_to_string(path)?;
    let content = raw.replace('\r', "");

    // Collect non-empty lines (strip the ~ terminator).
    let lines: Vec<&str> = content
        .lines()
        .map(|l| l.trim_end())
        .take_while(|l| l.trim() != "~")
        .collect();

    debug!(total_lines = lines.len(), "Lines before terminator");

    let mut cursor = Cursor::new(&lines);

    let animation_files = parse_animation_files(&mut cursor)?;
    let contract = parse_contract(&mut cursor)?;
    let negotiation = parse_negotiation(&mut cursor)?;
    let prestige = parse_prestige(&mut cursor)?;
    let intelligence = parse_intelligence(&mut cursor)?;
    let (enemy_count, npc_count, enemy_ratings) = parse_enemy_ratings(&mut cursor)?;
    let enemy_weapons = parse_enemy_weapons(&mut cursor, enemy_ratings.len())?;
    let preloaded_equipment = parse_equipment_line(&mut cursor, "preloaded")?;
    let (recommended_equipment, recommended_item) = parse_recommended_equipment(&mut cursor)?;
    let (start_hour, start_minute) = parse_start_time(&mut cursor)?;
    let weather = parse_weather(&mut cursor)?;
    let travel = parse_travel(&mut cursor)?;
    let special = parse_special(&mut cursor)?;

    let mission = Mission {
        animation_files,
        contract,
        negotiation,
        prestige,
        intelligence,
        enemy_count,
        npc_count,
        enemy_ratings,
        enemy_weapons,
        preloaded_equipment,
        recommended_equipment,
        recommended_item,
        start_hour,
        start_minute,
        weather,
        travel,
        special,
    };

    info!(
        enemies = mission.enemy_count,
        npcs = mission.npc_count,
        advance = mission.contract.advance,
        "Mission parsed successfully"
    );

    Ok(mission)
}

// ---------------------------------------------------------------------------
// Cursor helper -- walks lines forward, skipping blanks
// ---------------------------------------------------------------------------

struct Cursor<'a> {
    lines: &'a [&'a str],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(lines: &'a [&'a str]) -> Self {
        Self { lines, pos: 0 }
    }

    /// Advance past blank lines and return the next non-blank line.
    fn next_non_blank(&mut self) -> Option<&'a str> {
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            self.pos += 1;
            if !line.trim().is_empty() {
                return Some(line);
            }
        }
        None
    }

    /// Peek at the next non-blank line without consuming it.
    fn peek_non_blank(&self) -> Option<&'a str> {
        let mut p = self.pos;
        while p < self.lines.len() {
            let line = self.lines[p];
            p += 1;
            if !line.trim().is_empty() {
                return Some(line);
            }
        }
        None
    }

    /// Current 1-based line number (for error reporting).
    fn line_num(&self) -> usize {
        self.pos
    }

    /// Find the next line whose trimmed, lowercased form contains `needle` (lowercased).
    /// Consumes lines up to and including the match.
    fn find_label(&mut self, needle: &str) -> Result<&'a str, MissionError> {
        let needle_lower = needle.to_lowercase();
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            self.pos += 1;
            if line.to_lowercase().contains(&needle_lower) {
                trace!(line = self.pos, label = %needle, "Found label");
                return Ok(line);
            }
        }
        Err(MissionError::MissingSection {
            section: needle.to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_err(line: usize, msg: impl Into<String>) -> MissionError {
    MissionError::Parse {
        line,
        message: msg.into(),
    }
}

/// Extract the value portion after the first `:` in a line, trimmed.
fn after_colon(line: &str) -> &str {
    match line.find(':') {
        Some(i) => line[i + 1..].trim(),
        None => line.trim(),
    }
}

/// Parse a sprite filename, returning `None` for the literal string `null`.
fn parse_sprite(value: &str) -> Option<String> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("null") || v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

fn parse_u8(s: &str, line: usize, field: &str) -> Result<u8, MissionError> {
    s.parse::<u8>()
        .map_err(|_| parse_err(line, format!("invalid u8 for {field}: {s:?}")))
}

fn parse_i8(s: &str, line: usize, field: &str) -> Result<i8, MissionError> {
    s.parse::<i8>()
        .map_err(|_| parse_err(line, format!("invalid i8 for {field}: {s:?}")))
}

fn parse_u16(s: &str, line: usize, field: &str) -> Result<u16, MissionError> {
    s.parse::<u16>()
        .map_err(|_| parse_err(line, format!("invalid u16 for {field}: {s:?}")))
}

fn parse_i16(s: &str, line: usize, field: &str) -> Result<i16, MissionError> {
    s.parse::<i16>()
        .map_err(|_| parse_err(line, format!("invalid i16 for {field}: {s:?}")))
}

fn parse_u32(s: &str, line: usize, field: &str) -> Result<u32, MissionError> {
    s.parse::<u32>()
        .map_err(|_| parse_err(line, format!("invalid u32 for {field}: {s:?}")))
}

/// Split on whitespace and collect into a `Vec`.
fn ws_fields(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

// ---------------------------------------------------------------------------
// Section parsers
// ---------------------------------------------------------------------------

fn parse_animation_files(c: &mut Cursor) -> Result<AnimationFiles, MissionError> {
    c.find_label("animation files")?;

    let good_guys = {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: "Animation Files / Good Guys".into(),
        })?;
        after_colon(line).trim().to_string()
    };
    let bad_guys = {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: "Animation Files / Bad Guys".into(),
        })?;
        after_colon(line).trim().to_string()
    };
    let dogs = {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: "Animation Files / Dogs".into(),
        })?;
        parse_sprite(after_colon(line))
    };
    let npc1 = {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: "Animation Files / NPC1".into(),
        })?;
        parse_sprite(after_colon(line))
    };
    let npc2 = {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: "Animation Files / NPC2".into(),
        })?;
        parse_sprite(after_colon(line))
    };
    let npc3_vhc1 = {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: "Animation Files / NPC3/VHC1".into(),
        })?;
        parse_sprite(after_colon(line))
    };
    let npc4_vhc2 = {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: "Animation Files / NPC4/VHC2".into(),
        })?;
        parse_sprite(after_colon(line))
    };

    debug!(
        good_guys = %good_guys,
        bad_guys = %bad_guys,
        "Parsed animation files"
    );

    Ok(AnimationFiles {
        good_guys,
        bad_guys,
        dogs,
        npc1,
        npc2,
        npc3_vhc1,
        npc4_vhc2,
    })
}

fn parse_contract(c: &mut Cursor) -> Result<ContractTerms, MissionError> {
    c.find_label("contract:")?;

    // Date line
    let date_line = c.find_label("date:")?;
    let date_fields = ws_fields(after_colon(date_line));
    if date_fields.len() < 2 {
        return Err(parse_err(c.line_num(), "Date requires 2 values"));
    }
    let date_day = parse_u16(date_fields[0], c.line_num(), "date_day")?;
    let date_year = parse_u16(date_fields[1], c.line_num(), "date_year")?;

    // From line (label may or may not have content on same line)
    c.find_label("from")?;
    let from_line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
        section: "Contract / From".into(),
    })?;
    let from = from_line.trim().to_string();

    // Terms
    c.find_label("terms")?;
    let terms_line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
        section: "Contract / Terms".into(),
    })?;
    let terms = terms_line.trim().to_string();

    // Bonus text
    c.find_label("bonus:")?;
    let bonus_text_line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
        section: "Contract / Bonus text".into(),
    })?;
    let bonus_text = bonus_text_line.trim().to_string();

    // Advance/Bonus/Deadline values
    let abd_line = c.find_label("advance/bonus/deadline")?;
    let abd_fields = ws_fields(after_colon(abd_line));
    if abd_fields.len() < 4 {
        return Err(parse_err(
            c.line_num(),
            "Advance/Bonus/Deadline requires 4 values",
        ));
    }
    let advance = parse_u32(abd_fields[0], c.line_num(), "advance")?;
    let bonus = parse_u32(abd_fields[1], c.line_num(), "bonus")?;
    let deadline_day = parse_u16(abd_fields[2], c.line_num(), "deadline_day")?;
    let deadline_year = parse_u16(abd_fields[3], c.line_num(), "deadline_year")?;

    debug!(
        from = %from,
        advance = advance,
        bonus = bonus,
        "Parsed contract"
    );

    Ok(ContractTerms {
        date_day,
        date_year,
        from,
        terms,
        bonus_text,
        advance,
        bonus,
        deadline_day,
        deadline_year,
    })
}

fn parse_negotiation(c: &mut Cursor) -> Result<Negotiation, MissionError> {
    c.find_label("contract negotiation")?;

    // Player counter-offer ladder
    let adv_line = c.find_label("advance:")?;
    let adv_f = ws_fields(after_colon(adv_line));
    let mut advance = [0u32; 4];
    for (i, v) in adv_f.iter().take(4).enumerate() {
        advance[i] = parse_u32(v, c.line_num(), "negotiation advance")?;
    }

    let bon_line = c.find_label("bonus:")?;
    let bon_f = ws_fields(after_colon(bon_line));
    let mut bonus = [0u32; 4];
    for (i, v) in bon_f.iter().take(4).enumerate() {
        bonus[i] = parse_u32(v, c.line_num(), "negotiation bonus")?;
    }

    let dl_line = c.find_label("deadline:")?;
    let dl_f = ws_fields(after_colon(dl_line));
    let mut deadline = [0u16; 4];
    for (i, v) in dl_f.iter().take(4).enumerate() {
        deadline[i] = parse_u16(v, c.line_num(), "negotiation deadline")?;
    }

    let ch_line = c.find_label("chance:")?;
    let ch_f = ws_fields(after_colon(ch_line));
    let mut chance = [0u8; 4];
    for (i, v) in ch_f.iter().take(4).enumerate() {
        chance[i] = parse_u8(v, c.line_num(), "negotiation chance")?;
    }

    // AI counter-response table
    let counter_line = c.find_label("counter:")?;
    let ctr_f = ws_fields(after_colon(counter_line));
    let mut counter_values = [0u32; 8];
    for (i, v) in ctr_f.iter().take(8).enumerate() {
        counter_values[i] = parse_u32(v, c.line_num(), "counter values")?;
    }

    let ca_line = c.find_label("advance:")?;
    let ca_f = ws_fields(after_colon(ca_line));
    let mut counter_advance = [0u8; 8];
    for (i, v) in ca_f.iter().take(8).enumerate() {
        counter_advance[i] = parse_u8(v, c.line_num(), "counter advance")?;
    }

    let cb_line = c.find_label("bonus:")?;
    let cb_f = ws_fields(after_colon(cb_line));
    let mut counter_bonus = [0u8; 8];
    for (i, v) in cb_f.iter().take(8).enumerate() {
        counter_bonus[i] = parse_u8(v, c.line_num(), "counter bonus")?;
    }

    let cd_line = c.find_label("deadline:")?;
    let cd_f = ws_fields(after_colon(cd_line));
    let mut counter_deadline = [0u8; 8];
    for (i, v) in cd_f.iter().take(8).enumerate() {
        counter_deadline[i] = parse_u8(v, c.line_num(), "counter deadline")?;
    }

    debug!(?advance, ?bonus, ?deadline, ?chance, "Parsed negotiation");

    Ok(Negotiation {
        advance,
        bonus,
        deadline,
        chance,
        counter_values,
        counter_advance,
        counter_bonus,
        counter_deadline,
    })
}

fn parse_prestige(c: &mut Cursor) -> Result<PrestigeConfig, MissionError> {
    c.find_label("prestige:")?;

    let line = c.find_label("mission type")?;
    let fields = ws_fields(after_colon(line));
    if fields.len() < 8 {
        return Err(parse_err(c.line_num(), "Prestige requires 8 values"));
    }

    let prestige = PrestigeConfig {
        mission_type: parse_u8(fields[0], c.line_num(), "mission_type")?,
        entrance: parse_u8(fields[1], c.line_num(), "entrance")?,
        num_maps: parse_u8(fields[2], c.line_num(), "num_maps")?,
        success1: parse_i16(fields[3], c.line_num(), "success1")?,
        success2: parse_i16(fields[4], c.line_num(), "success2")?,
        wia: parse_i8(fields[5], c.line_num(), "wia")?,
        mia: parse_i8(fields[6], c.line_num(), "mia")?,
        kia: parse_i8(fields[7], c.line_num(), "kia")?,
    };

    debug!(
        mission_type = prestige.mission_type,
        success1 = prestige.success1,
        "Parsed prestige"
    );

    Ok(prestige)
}

fn parse_intelligence(c: &mut Cursor) -> Result<IntelligenceConfig, MissionError> {
    c.find_label("intelligence:")?;

    // Three tiers
    let mut tiers: Vec<IntelTier> = Vec::with_capacity(3);
    let tier_names = [
        "Information Consultants",
        "Intelligence, Inc",
        "Global Intelligence",
    ];

    for name in &tier_names {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: format!("Intelligence / {name}"),
        })?;
        let fields = ws_fields(after_colon(line));
        if fields.len() < 2 {
            return Err(parse_err(
                c.line_num(),
                format!("Intel tier {name} requires 2 values"),
            ));
        }
        tiers.push(IntelTier {
            name: name.to_string(),
            cost: parse_u32(fields[0], c.line_num(), "intel cost")?,
            per_item: parse_u32(fields[1], c.line_num(), "intel per_item")?,
        });
    }

    // Summary stats line
    let stats_line = c.find_label("men/exp")?;
    let stats_fields = ws_fields(after_colon(stats_line));
    if stats_fields.len() < 6 {
        return Err(parse_err(c.line_num(), "Intel stats requires 6 values"));
    }

    // Attachments
    let attach_line = c.find_label("attachments")?;
    let attachments = parse_u8(after_colon(attach_line).trim(), c.line_num(), "attachments")?;

    let tiers_arr: [IntelTier; 3] = [tiers.remove(0), tiers.remove(0), tiers.remove(0)];

    debug!(attachments = attachments, "Parsed intelligence");

    Ok(IntelligenceConfig {
        tiers: tiers_arr,
        men: parse_u8(stats_fields[0], c.line_num(), "men")?,
        exp: parse_u8(stats_fields[1], c.line_num(), "exp")?,
        fire_power: parse_u8(stats_fields[2], c.line_num(), "fire_power")?,
        success: parse_u8(stats_fields[3], c.line_num(), "success")?,
        casualties: parse_u8(stats_fields[4], c.line_num(), "casualties")?,
        scene_type: parse_u8(stats_fields[5], c.line_num(), "scene_type")?,
        attachments,
    })
}

fn parse_enemy_ratings(c: &mut Cursor) -> Result<(u16, u16, Vec<EnemyRating>), MissionError> {
    c.find_label("enemy ratings chart")?;

    let num_line = c.find_label("number:")?;
    let enemy_count = parse_u16(
        after_colon(num_line)
            .split_whitespace()
            .next()
            .unwrap_or("0"),
        c.line_num(),
        "enemy count",
    )?;

    let npc_line = c.find_label("npcs:")?;
    let npc_count = parse_u16(
        after_colon(npc_line)
            .split_whitespace()
            .next()
            .unwrap_or("0"),
        c.line_num(),
        "npc count",
    )?;

    let total_rows = (enemy_count + npc_count) as usize;
    debug!(enemy_count, npc_count, total_rows, "Enemy ratings header");

    // Skip the column header line (Rating  DPR  EXP ...)
    let header_line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
        section: "Enemy Ratings / header row".into(),
    })?;
    trace!(header = %header_line, "Skipping ratings column header");

    let mut ratings = Vec::with_capacity(total_rows);
    for i in 0..total_rows {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: format!("Enemy Ratings / row {i}"),
        })?;
        let f = ws_fields(line);
        if f.len() < 13 {
            return Err(parse_err(
                c.line_num(),
                format!("Enemy rating row {i} has {} fields, expected 13", f.len()),
            ));
        }
        ratings.push(EnemyRating {
            rating: parse_u8(f[0], c.line_num(), "rating")?,
            dpr: parse_u8(f[1], c.line_num(), "dpr")?,
            exp: parse_u8(f[2], c.line_num(), "exp")?,
            str_: parse_u8(f[3], c.line_num(), "str")?,
            agl: parse_u8(f[4], c.line_num(), "agl")?,
            wil: parse_u8(f[5], c.line_num(), "wil")?,
            wsk: parse_u8(f[6], c.line_num(), "wsk")?,
            hhc: parse_u8(f[7], c.line_num(), "hhc")?,
            tch: parse_u8(f[8], c.line_num(), "tch")?,
            enc: parse_u16(f[9], c.line_num(), "enc")?,
            aps: parse_u8(f[10], c.line_num(), "aps")?,
            presence_chance: parse_u8(f[11], c.line_num(), "presence_chance")?,
            enemy_type: parse_u8(f[12], c.line_num(), "enemy_type")?,
        });
        trace!(
            row = i,
            rating = ratings.last().unwrap().rating,
            "Parsed enemy rating"
        );
    }

    if ratings.len() != total_rows {
        return Err(MissionError::EnemyRowCount {
            expected: total_rows,
            found: ratings.len(),
        });
    }

    Ok((enemy_count, npc_count, ratings))
}

fn parse_enemy_weapons(
    c: &mut Cursor,
    expected_rows: usize,
) -> Result<Vec<EnemyWeapon>, MissionError> {
    c.find_label("enemy weapons chart")?;

    let mut weapons = Vec::with_capacity(expected_rows);
    for i in 0..expected_rows {
        let line = c.next_non_blank().ok_or(MissionError::UnexpectedEof {
            section: format!("Enemy Weapons / row {i}"),
        })?;
        let f = ws_fields(line);
        if f.len() < 6 {
            return Err(parse_err(
                c.line_num(),
                format!("Enemy weapon row {i} has {} fields, expected 6", f.len()),
            ));
        }
        weapons.push(EnemyWeapon {
            weapon1: parse_i8(f[0], c.line_num(), "weapon1")?,
            weapon2: parse_i8(f[1], c.line_num(), "weapon2")?,
            ammo1: parse_u8(f[2], c.line_num(), "ammo1")?,
            ammo2: parse_u8(f[3], c.line_num(), "ammo2")?,
            weapon3: parse_i8(f[4], c.line_num(), "weapon3")?,
            extra: parse_i8(f[5], c.line_num(), "extra")?,
        });
        trace!(
            row = i,
            w1 = weapons.last().unwrap().weapon1,
            "Parsed enemy weapon"
        );
    }

    if weapons.len() != expected_rows {
        return Err(MissionError::EnemyRowCount {
            expected: expected_rows,
            found: weapons.len(),
        });
    }

    Ok(weapons)
}

fn parse_equipment_line(c: &mut Cursor, label: &str) -> Result<EquipmentCounts, MissionError> {
    let line = c.find_label("equipment")?;
    let fields = ws_fields(after_colon(line));
    if fields.len() < 3 {
        return Err(parse_err(
            c.line_num(),
            format!("{label} equipment requires 3 values"),
        ));
    }
    debug!(%label, ?fields, "Parsed equipment line");
    Ok(EquipmentCounts {
        weapons: parse_u8(fields[0], c.line_num(), "weapons")?,
        ammo: parse_u8(fields[1], c.line_num(), "ammo")?,
        equipment: parse_u8(fields[2], c.line_num(), "equipment")?,
    })
}

fn parse_recommended_equipment(
    c: &mut Cursor,
) -> Result<(EquipmentCounts, Option<RecommendedItem>), MissionError> {
    let line = c.find_label("recommended equipment")?;
    let fields = ws_fields(after_colon(line));
    if fields.len() < 3 {
        return Err(parse_err(
            c.line_num(),
            "Recommended equipment requires 3 values",
        ));
    }
    let counts = EquipmentCounts {
        weapons: parse_u8(fields[0], c.line_num(), "rec weapons")?,
        ammo: parse_u8(fields[1], c.line_num(), "rec ammo")?,
        equipment: parse_u8(fields[2], c.line_num(), "rec equipment")?,
    };

    // Check if there's an Equipment/Equip Amount/Number line
    let item = if counts.weapons > 0 || counts.ammo > 0 || counts.equipment > 0 {
        if let Some(peek) = c.peek_non_blank() {
            let peek_lower = peek.to_lowercase();
            if peek_lower.contains("amount") || peek_lower.contains("equip") {
                let item_line = c.next_non_blank().unwrap();
                let item_fields = ws_fields(after_colon(item_line));
                if item_fields.len() >= 2 {
                    Some(RecommendedItem {
                        item_id: parse_u16(item_fields[0], c.line_num(), "rec item_id")?,
                        count: parse_u16(item_fields[1], c.line_num(), "rec count")?,
                    })
                } else {
                    warn!(
                        line = c.line_num(),
                        "Equipment Amount/Number line has fewer than 2 fields"
                    );
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    debug!(?counts, ?item, "Parsed recommended equipment");

    Ok((counts, item))
}

fn parse_start_time(c: &mut Cursor) -> Result<(u8, u8), MissionError> {
    let line = c.find_label("start time")?;
    let fields = ws_fields(after_colon(line));
    if fields.len() < 2 {
        return Err(parse_err(c.line_num(), "Start Time requires 2 values"));
    }
    let hour = parse_u8(fields[0], c.line_num(), "start hour")?;
    let minute = parse_u8(fields[1], c.line_num(), "start minute")?;
    debug!(hour, minute, "Parsed start time");
    Ok((hour, minute))
}

fn parse_weather(c: &mut Cursor) -> Result<WeatherTable, MissionError> {
    c.find_label("weather table")?;
    let line = c.find_label("clear")?;
    let fields = ws_fields(after_colon(line));
    if fields.len() < 6 {
        return Err(parse_err(c.line_num(), "Weather table requires 6 values"));
    }

    let weather = WeatherTable {
        clear: parse_u8(fields[0], c.line_num(), "clear")?,
        foggy: parse_u8(fields[1], c.line_num(), "foggy")?,
        overcast: parse_u8(fields[2], c.line_num(), "overcast")?,
        light_rain: parse_u8(fields[3], c.line_num(), "light_rain")?,
        heavy_rain: parse_u8(fields[4], c.line_num(), "heavy_rain")?,
        storm: parse_u8(fields[5], c.line_num(), "storm")?,
    };

    debug!(
        clear = weather.clear,
        foggy = weather.foggy,
        "Parsed weather table"
    );

    Ok(weather)
}

fn parse_travel(c: &mut Cursor) -> Result<TravelTable, MissionError> {
    c.find_label("travel table")?;
    let line = c.find_label("cost1")?;
    let fields = ws_fields(after_colon(line));
    if fields.len() < 6 {
        return Err(parse_err(c.line_num(), "Travel table requires 6 values"));
    }

    let travel = TravelTable {
        cost1: parse_u32(fields[0], c.line_num(), "cost1")?,
        cost2: parse_u32(fields[1], c.line_num(), "cost2")?,
        cost3: parse_u32(fields[2], c.line_num(), "cost3")?,
        days1: parse_u8(fields[3], c.line_num(), "days1")?,
        days2: parse_u8(fields[4], c.line_num(), "days2")?,
        days3: parse_u8(fields[5], c.line_num(), "days3")?,
    };

    debug!(
        cost1 = travel.cost1,
        days1 = travel.days1,
        "Parsed travel table"
    );

    Ok(travel)
}

fn parse_special(c: &mut Cursor) -> Result<SpecialConfig, MissionError> {
    let turns_line = c.find_label("special turns")?;
    let turns = parse_u8(
        after_colon(turns_line)
            .split_whitespace()
            .next()
            .unwrap_or("0"),
        c.line_num(),
        "special turns",
    )?;

    let type_line = c.find_label("special type")?;
    let special_type = parse_u8(
        after_colon(type_line)
            .split_whitespace()
            .next()
            .unwrap_or("0"),
        c.line_num(),
        "special type",
    )?;

    let item_line = c.find_label("special item")?;
    let item = parse_u8(
        after_colon(item_line)
            .split_whitespace()
            .next()
            .unwrap_or("0"),
        c.line_num(),
        "special item",
    )?;

    let dmg_line = c.find_label("special damage")?;
    let damage = parse_u8(
        after_colon(dmg_line)
            .split_whitespace()
            .next()
            .unwrap_or("0"),
        c.line_num(),
        "special damage",
    )?;

    let damage_message = if damage > 0 {
        c.next_non_blank().map(|l| l.trim().to_string())
    } else {
        None
    };

    debug!(
        turns,
        special_type,
        item,
        damage,
        ?damage_message,
        "Parsed special config"
    );

    Ok(SpecialConfig {
        turns,
        special_type,
        item,
        damage,
        damage_message,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a minimal synthetic mission file for testing.
    fn synthetic_mission() -> String {
        r#"Animation Files:
Good Guys: jungsld.cor
Bad Guys:  jungemy.cor
Dogs:      guarddog.cor
NPC1:      woman.cor
NPC2:      null
NPC3/VHC1: copter01.cor
NPC4/VHC2: null

Contract:
Date:  7 2001
From:
Richarde LeClure, President and CEO, ADI

Terms:
Rescue the hostage.
Bonus:
Return hostage alive by deadline.
Advance/Bonus/Deadline:  324000 535000 20 2001

Contract Negotiation:
Advance:  349000 374000 399000 424000
Bonus:    560000 585000 610000 635000
Deadline:     22     24     26     28
Chance:       76     52     28     04

Counter:  25000 50000 75000 100000  2  4  6  8
Advance:     10    40    10     40 10 30 10 30
Bonus:       10    80    10     70 10 50 10 40
Deadline:    10    80    10     70 10 60 10 50

Prestige:
Mission Type/Entrance/# MAPS/Success1/Success2/WIA/MIA/KIA: 1 1 1 20 0 -1 -2 -2

Intelligence:
Information Consultants:  40000  5000
Intelligence, Inc:        70000  7500
Global Intelligence:     100000 10000
Men/Exp/FirePower/Success/Casualties/Scene Type: 5 2 1 85 1 1

Attachments: 2

Enemy Ratings Chart:
Number:  3
NPCs:    1
Rating  DPR  EXP  STR  AGL  WIL  WSK  HHC  TCH  ENC  APS  There  Type
  9  133   5   59   33    6   23   30   13  300   30   100    2
 10  130   5   51   43    8   30   32   89  300   32    20    2
 12  118   7   26   60   10   42   33   16  300   32   100    2
 14  118  12   23   43   12   32   27   54  225   32   100    3

Enemy Weapons Chart:   Weapon 1/Weapon 2/Ammo 1/Ammo 2/Weapon 3
 19   9   2   1  -1    5
 22   0   2   1  44    0
 21   6   8   2  -1   12
 -1  -1   0   0  -1   -1

PreLoaded Equipment (Weapons/Ammo/Equipment): 0 0 0

Recommended Equipment (Weapons/Ammo/Equipment): 0 0 1
Equip Amount/Number:   5 1

Start Time: 10 0

Weather Table:
Clear/Foggy/OverCast/LtRain/HvyRain/Storm: 10 10 50 30 0 0

Travel Table:
Cost1/Cost2/Cost3/Days1/Days2/Days3: 20000 30000 50000 5 4 3

Special Turns (# Turns to Complete Action): 0
Special Type: 0
Special Item: 0

Special Damage: 2
%s has been bitten by a snake!
~
"#
        .to_string()
    }

    fn write_temp_file(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_parse_synthetic_mission() {
        let file = write_temp_file(&synthetic_mission());
        let mission = parse_mission(file.path()).expect("should parse synthetic mission");

        // Animation files
        assert_eq!(mission.animation_files.good_guys, "jungsld.cor");
        assert_eq!(mission.animation_files.bad_guys, "jungemy.cor");
        assert_eq!(
            mission.animation_files.dogs.as_deref(),
            Some("guarddog.cor")
        );
        assert_eq!(mission.animation_files.npc1.as_deref(), Some("woman.cor"));
        assert!(mission.animation_files.npc2.is_none());
        assert_eq!(
            mission.animation_files.npc3_vhc1.as_deref(),
            Some("copter01.cor")
        );
        assert!(mission.animation_files.npc4_vhc2.is_none());

        // Contract
        assert_eq!(mission.contract.date_day, 7);
        assert_eq!(mission.contract.date_year, 2001);
        assert!(mission.contract.from.contains("Richarde LeClure"));
        assert_eq!(mission.contract.advance, 324_000);
        assert_eq!(mission.contract.bonus, 535_000);
        assert_eq!(mission.contract.deadline_day, 20);

        // Negotiation
        assert_eq!(
            mission.negotiation.advance,
            [349000, 374000, 399000, 424000]
        );
        assert_eq!(mission.negotiation.chance, [76, 52, 28, 4]);
        assert_eq!(
            mission.negotiation.counter_values,
            [25000, 50000, 75000, 100000, 2, 4, 6, 8]
        );

        // Prestige
        assert_eq!(mission.prestige.mission_type, 1);
        assert_eq!(mission.prestige.success1, 20);
        assert_eq!(mission.prestige.wia, -1);
        assert_eq!(mission.prestige.kia, -2);

        // Intelligence
        assert_eq!(mission.intelligence.tiers[0].cost, 40_000);
        assert_eq!(mission.intelligence.tiers[2].cost, 100_000);
        assert_eq!(mission.intelligence.men, 5);
        assert_eq!(mission.intelligence.scene_type, 1);
        assert_eq!(mission.intelligence.attachments, 2);

        // Enemy ratings
        assert_eq!(mission.enemy_count, 3);
        assert_eq!(mission.npc_count, 1);
        assert_eq!(mission.enemy_ratings.len(), 4);
        assert_eq!(mission.enemy_ratings[0].rating, 9);
        assert_eq!(mission.enemy_ratings[0].dpr, 133);
        assert_eq!(mission.enemy_ratings[3].enemy_type, 3); // NPC

        // Enemy weapons
        assert_eq!(mission.enemy_weapons.len(), 4);
        assert_eq!(mission.enemy_weapons[0].weapon1, 19);
        assert_eq!(mission.enemy_weapons[0].extra, 5);
        assert_eq!(mission.enemy_weapons[3].weapon1, -1); // NPC unarmed

        // Equipment
        assert_eq!(mission.preloaded_equipment.weapons, 0);
        assert_eq!(mission.recommended_equipment.equipment, 1);
        assert!(mission.recommended_item.is_some());
        assert_eq!(mission.recommended_item.as_ref().unwrap().item_id, 5);
        assert_eq!(mission.recommended_item.as_ref().unwrap().count, 1);

        // Start time
        assert_eq!(mission.start_hour, 10);
        assert_eq!(mission.start_minute, 0);

        // Weather
        assert_eq!(mission.weather.clear, 10);
        assert_eq!(mission.weather.foggy, 10);
        assert_eq!(mission.weather.overcast, 50);
        assert_eq!(mission.weather.light_rain, 30);

        // Travel
        assert_eq!(mission.travel.cost1, 20_000);
        assert_eq!(mission.travel.days3, 3);

        // Special
        assert_eq!(mission.special.damage, 2);
        assert_eq!(
            mission.special.damage_message.as_deref(),
            Some("%s has been bitten by a snake!")
        );
    }

    #[test]
    fn test_parse_all_zeros_negotiation() {
        // MSSN16-style non-negotiable contract
        let content = synthetic_mission().replace(
            "Advance:  349000 374000 399000 424000\n\
             Bonus:    560000 585000 610000 635000\n\
             Deadline:     22     24     26     28\n\
             Chance:       76     52     28     04\n\
             \n\
             Counter:  25000 50000 75000 100000  2  4  6  8\n\
             Advance:     10    40    10     40 10 30 10 30\n\
             Bonus:       10    80    10     70 10 50 10 40\n\
             Deadline:    10    80    10     70 10 60 10 50",
            "Advance:  0 0 0 0\n\
             Bonus:    0 0 0 0\n\
             Deadline: 0 0 0 0\n\
             Chance:   0 0 0 0\n\
             \n\
             Counter:  0 0 0 0  0  0  0  0\n\
             Advance:  0 0 0 0  0  0  0  0\n\
             Bonus:    0 0 0 0  0  0  0  0\n\
             Deadline: 0 0 0 0  0  0  0  0",
        );
        let file = write_temp_file(&content);
        let mission = parse_mission(file.path()).expect("should parse zero negotiation");
        assert_eq!(mission.negotiation.advance, [0, 0, 0, 0]);
        assert_eq!(mission.negotiation.chance, [0, 0, 0, 0]);
        assert_eq!(mission.negotiation.counter_values, [0; 8]);
    }

    #[test]
    fn test_no_special_damage_message() {
        let content = synthetic_mission().replace(
            "Special Damage: 2\n%s has been bitten by a snake!",
            "Special Damage: 0",
        );
        let file = write_temp_file(&content);
        let mission = parse_mission(file.path()).expect("should parse no damage");
        assert_eq!(mission.special.damage, 0);
        assert!(mission.special.damage_message.is_none());
    }

    #[test]
    fn test_no_recommended_item() {
        let content = synthetic_mission().replace(
            "Recommended Equipment (Weapons/Ammo/Equipment): 0 0 1\n\
             Equip Amount/Number:   5 1",
            "Recommended Equipment (Weapons/Ammo/Equipment): 0 0 0",
        );
        let file = write_temp_file(&content);
        let mission = parse_mission(file.path()).expect("should parse no rec item");
        assert_eq!(mission.recommended_equipment.equipment, 0);
        assert!(mission.recommended_item.is_none());
    }

    #[test]
    fn test_crlf_handling() {
        let content = synthetic_mission().replace('\n', "\r\n");
        let file = write_temp_file(&content);
        let mission = parse_mission(file.path()).expect("should handle CRLF");
        assert_eq!(mission.animation_files.good_guys, "jungsld.cor");
    }

    #[test]
    fn test_missing_section_error() {
        let content = "Animation Files:\nGood Guys: test.cor\n~\n";
        let file = write_temp_file(content);
        let result = parse_mission(file.path());
        assert!(result.is_err());
        match result.unwrap_err() {
            MissionError::MissingSection { section } | MissionError::UnexpectedEof { section } => {
                // Expected -- some required section is missing
                assert!(!section.is_empty());
            }
            other => panic!("Expected MissingSection or UnexpectedEof, got: {other:?}"),
        }
    }

    #[test]
    fn test_equipment_amount_number_variant() {
        // Test the "Equipment Amount/Number:" variant (MSSN08/16 style)
        let content = synthetic_mission().replace(
            "Equip Amount/Number:   5 1",
            "Equipment Amount/Number: 49 1",
        );
        let file = write_temp_file(&content);
        let mission = parse_mission(file.path()).expect("should parse Equipment Amount variant");
        assert_eq!(mission.recommended_item.as_ref().unwrap().item_id, 49);
    }
}
