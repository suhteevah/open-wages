//! Parser for `MERCS.DAT` — the master mercenary roster file.
//!
//! Each record contains a mercenary's identity, attributes, hiring costs, and biography.
//! Records are separated by `<` delimiter lines. The file terminates with `~` sentinel lines.

use std::path::Path;

use thiserror::Error;
use tracing::{debug, info, trace};

/// Errors that can occur when parsing `MERCS.DAT`.
#[derive(Debug, Error)]
pub enum MercsError {
    #[error("I/O error reading MERCS.DAT: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("validation error for mercenary '{name}': {message}")]
    Validation { name: String, message: String },
}

/// A single mercenary record parsed from `MERCS.DAT`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Mercenary {
    /// Full name (underscores converted to spaces).
    pub name: String,
    /// Short display name / callsign.
    pub nickname: String,
    /// Age in years.
    pub age: u32,
    /// Height — feet component.
    pub height_feet: u32,
    /// Height — inches component.
    pub height_inches: u32,
    /// Weight in pounds.
    pub weight: u32,
    /// Nationality / country of origin.
    pub nation: String,
    /// Overall mercenary rating (composite score).
    pub rating: i32,
    /// Daily Pay Rate.
    pub dpr: i32,
    /// Prestige (can be negative).
    pub psg: i32,
    /// Availability flag (1 = available, 0 = unavailable).
    pub avail: i32,
    /// Experience level.
    pub exp: i32,
    /// Strength.
    pub str_stat: i32,
    /// Agility.
    pub agl: i32,
    /// Willpower.
    pub wil: i32,
    /// Weapon Skill.
    pub wsk: i32,
    /// Hand-to-Hand Combat.
    pub hhc: i32,
    /// Tech aptitude.
    pub tch: i32,
    /// Encumbrance capacity.
    pub enc: i32,
    /// Action Points per combat turn.
    pub aps: i32,
    /// Hiring fee tier 1.
    pub fee_hire: i32,
    /// Hiring fee tier 2.
    pub fee_bonus: i32,
    /// Hiring fee tier 3.
    pub fee_death: i32,
    /// Mail flag (1 = has intro mail).
    pub mail: i32,
    /// Biography / backstory text.
    pub biography: String,
}

/// Convert underscore-delimited names to spaces.
fn underscores_to_spaces(s: &str) -> String {
    s.replace('_', " ")
}

/// Strip the `\r` from a line (handles CR/LF endings).
fn strip_cr(line: &str) -> &str {
    line.trim_end_matches('\r')
}

/// Extract the value portion after a `Label:` prefix, trimmed.
/// Returns `None` if the prefix is not found.
fn extract_after(line: &str, prefix: &str) -> Option<String> {
    let idx = line.find(prefix)?;
    Some(line[idx + prefix.len()..].trim().to_string())
}

/// Parse an integer from a string, returning a `MercsError::Parse` on failure.
fn parse_int(s: &str, line_num: usize, field: &str) -> Result<i32, MercsError> {
    s.trim().parse::<i32>().map_err(|_| MercsError::Parse {
        line: line_num,
        message: format!(
            "failed to parse '{}' as integer for field '{}'",
            s.trim(),
            field
        ),
    })
}

/// Parse a `u32` from a string, returning a `MercsError::Parse` on failure.
fn parse_uint(s: &str, line_num: usize, field: &str) -> Result<u32, MercsError> {
    s.trim().parse::<u32>().map_err(|_| MercsError::Parse {
        line: line_num,
        message: format!(
            "failed to parse '{}' as unsigned integer for field '{}'",
            s.trim(),
            field
        ),
    })
}

/// Parse a single mercenary record from a slice of lines.
///
/// `base_line` is the 1-based line number of the first line in the file,
/// used for error reporting.
fn parse_record(lines: &[&str], base_line: usize) -> Result<Mercenary, MercsError> {
    // We need to find specific lines by their prefix, skipping blank lines.
    // Collect non-blank lines with their file line numbers.
    let content_lines: Vec<(usize, &str)> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| (base_line + i, *l))
        .filter(|(_, l)| !l.trim().is_empty())
        .collect();

    if content_lines.len() < 10 {
        return Err(MercsError::Parse {
            line: base_line,
            message: format!(
                "record has only {} non-blank lines, expected at least 10",
                content_lines.len()
            ),
        });
    }

    // Line 1: Name:  <full_name>
    let (ln, line) = content_lines[0];
    let raw_name = extract_after(line, "Name:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'Name:' prefix".into(),
    })?;
    let name = underscores_to_spaces(&raw_name);
    trace!(line = ln, raw = %raw_name, parsed = %name, "parsed name");

    // Line 2: Nickname:  <nickname>
    let (ln, line) = content_lines[1];
    let raw_nick = extract_after(line, "Nickname:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'Nickname:' prefix".into(),
    })?;
    let nickname = underscores_to_spaces(&raw_nick);
    trace!(line = ln, %nickname, "parsed nickname");

    // Line 3: Age:  <age>\tHgt:  <feet> <inches>\tWgt:  <weight> lbs.
    let (ln, line) = content_lines[2];
    let age_str = extract_after(line, "Age:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'Age:' prefix".into(),
    })?;
    // Split on "Hgt:" to isolate age
    let (age_part, rest) = age_str
        .split_once("Hgt:")
        .ok_or_else(|| MercsError::Parse {
            line: ln,
            message: "expected 'Hgt:' on age/height/weight line".into(),
        })?;
    let age = parse_uint(age_part, ln, "Age")?;

    // rest is something like "  5 6\tWgt:  130 lbs."
    let (hgt_part, wgt_part) = rest.split_once("Wgt:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'Wgt:' on age/height/weight line".into(),
    })?;
    let hgt_tokens: Vec<&str> = hgt_part.split_whitespace().collect();
    if hgt_tokens.len() < 2 {
        return Err(MercsError::Parse {
            line: ln,
            message: format!(
                "expected 2 height tokens (feet inches), got {}",
                hgt_tokens.len()
            ),
        });
    }
    let height_feet = parse_uint(hgt_tokens[0], ln, "height_feet")?;
    let height_inches = parse_uint(hgt_tokens[1], ln, "height_inches")?;

    // Weight: strip "lbs." suffix
    let weight_str = wgt_part.trim().trim_end_matches("lbs.").trim();
    let weight = parse_uint(weight_str, ln, "weight")?;
    trace!(line = ln, %age, %height_feet, %height_inches, %weight, "parsed age/hgt/wgt");

    // Line 4: Nation:  <nation>
    let (ln, line) = content_lines[3];
    let raw_nation = extract_after(line, "Nation:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'Nation:' prefix".into(),
    })?;
    let nation = underscores_to_spaces(&raw_nation);
    trace!(line = ln, %nation, "parsed nation");

    // Line 5 (content_lines[4]): Missions: ... — skip this line
    // Line 6 (content_lines[5]): RATING: ... DPR: ... PSG: ... AVAIL: ...
    let rating_idx = content_lines
        .iter()
        .position(|(_, l)| l.contains("RATING:"))
        .ok_or_else(|| MercsError::Parse {
            line: base_line,
            message: "could not find RATING: line in record".into(),
        })?;
    let (ln, line) = content_lines[rating_idx];

    let rating_val = extract_after(line, "RATING:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'RATING:' prefix".into(),
    })?;
    // rating_val is like "76             DPR:  142       PSG:  280         AVAIL: 1"
    let (rating_str, rest) = rating_val
        .split_once("DPR:")
        .ok_or_else(|| MercsError::Parse {
            line: ln,
            message: "expected 'DPR:' on RATING line".into(),
        })?;
    let rating = parse_int(rating_str, ln, "RATING")?;

    let (dpr_str, rest) = rest.split_once("PSG:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'PSG:' on RATING line".into(),
    })?;
    let dpr = parse_int(dpr_str, ln, "DPR")?;

    let (psg_str, avail_str) = rest.split_once("AVAIL:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'AVAIL:' on RATING line".into(),
    })?;
    let psg = parse_int(psg_str, ln, "PSG")?;
    let avail = parse_int(avail_str, ln, "AVAIL")?;
    trace!(line = ln, %rating, %dpr, %psg, %avail, "parsed rating line");

    // EXP/STR/AGL line
    let exp_idx = content_lines
        .iter()
        .position(|(_, l)| l.contains("EXP:"))
        .ok_or_else(|| MercsError::Parse {
            line: base_line,
            message: "could not find EXP: line in record".into(),
        })?;
    let (ln, line) = content_lines[exp_idx];
    let exp_val = extract_after(line, "EXP:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'EXP:' prefix".into(),
    })?;
    let (exp_str, rest) = exp_val
        .split_once("STR:")
        .ok_or_else(|| MercsError::Parse {
            line: ln,
            message: "expected 'STR:' on EXP line".into(),
        })?;
    let exp = parse_int(exp_str, ln, "EXP")?;
    let (str_str, agl_str) = rest.split_once("AGL:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'AGL:' on EXP line".into(),
    })?;
    let str_stat = parse_int(str_str, ln, "STR")?;
    let agl = parse_int(agl_str, ln, "AGL")?;
    trace!(line = ln, %exp, %str_stat, %agl, "parsed EXP/STR/AGL");

    // WIL/WSK/HHC line
    let wil_idx = content_lines
        .iter()
        .position(|(_, l)| l.contains("WIL:"))
        .ok_or_else(|| MercsError::Parse {
            line: base_line,
            message: "could not find WIL: line in record".into(),
        })?;
    let (ln, line) = content_lines[wil_idx];
    let wil_val = extract_after(line, "WIL:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'WIL:' prefix".into(),
    })?;
    let (wil_str, rest) = wil_val
        .split_once("WSK:")
        .ok_or_else(|| MercsError::Parse {
            line: ln,
            message: "expected 'WSK:' on WIL line".into(),
        })?;
    let wil = parse_int(wil_str, ln, "WIL")?;
    let (wsk_str, hhc_str) = rest.split_once("HHC:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'HHC:' on WIL line".into(),
    })?;
    let wsk = parse_int(wsk_str, ln, "WSK")?;
    let hhc = parse_int(hhc_str, ln, "HHC")?;
    trace!(line = ln, %wil, %wsk, %hhc, "parsed WIL/WSK/HHC");

    // TCH/ENC/APS line
    let tch_idx = content_lines
        .iter()
        .position(|(_, l)| l.contains("TCH:"))
        .ok_or_else(|| MercsError::Parse {
            line: base_line,
            message: "could not find TCH: line in record".into(),
        })?;
    let (ln, line) = content_lines[tch_idx];
    let tch_val = extract_after(line, "TCH:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'TCH:' prefix".into(),
    })?;
    let (tch_str, rest) = tch_val
        .split_once("ENC:")
        .ok_or_else(|| MercsError::Parse {
            line: ln,
            message: "expected 'ENC:' on TCH line".into(),
        })?;
    let tch = parse_int(tch_str, ln, "TCH")?;
    let (enc_str, aps_str) = rest.split_once("APS:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'APS:' on TCH line".into(),
    })?;
    let enc = parse_int(enc_str, ln, "ENC")?;
    let aps = parse_int(aps_str, ln, "APS")?;
    trace!(line = ln, %tch, %enc, %aps, "parsed TCH/ENC/APS");

    // Fees line
    let fees_idx = content_lines
        .iter()
        .position(|(_, l)| l.contains("Fees:"))
        .ok_or_else(|| MercsError::Parse {
            line: base_line,
            message: "could not find Fees: line in record".into(),
        })?;
    let (ln, line) = content_lines[fees_idx];
    let fees_val = extract_after(line, "Fees:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'Fees:' prefix".into(),
    })?;
    let fee_tokens: Vec<&str> = fees_val.split_whitespace().collect();
    if fee_tokens.len() < 3 {
        return Err(MercsError::Parse {
            line: ln,
            message: format!("expected 3 fee values, got {}", fee_tokens.len()),
        });
    }
    let fee_hire = parse_int(fee_tokens[0], ln, "fee_hire")?;
    let fee_bonus = parse_int(fee_tokens[1], ln, "fee_bonus")?;
    let fee_death = parse_int(fee_tokens[2], ln, "fee_death")?;
    trace!(line = ln, %fee_hire, %fee_bonus, %fee_death, "parsed fees");

    // mail line
    let mail_idx = content_lines
        .iter()
        .position(|(_, l)| l.trim_start().starts_with("mail:"))
        .ok_or_else(|| MercsError::Parse {
            line: base_line,
            message: "could not find 'mail:' line in record".into(),
        })?;
    let (ln, line) = content_lines[mail_idx];
    let mail_val = extract_after(line, "mail:").ok_or_else(|| MercsError::Parse {
        line: ln,
        message: "expected 'mail:' prefix".into(),
    })?;
    let mail = parse_int(&mail_val, ln, "mail")?;
    trace!(line = ln, %mail, "parsed mail");

    // Biography: everything after the mail line (next non-blank content line)
    let biography = if mail_idx + 1 < content_lines.len() {
        content_lines[mail_idx + 1].1.trim().to_string()
    } else {
        String::new()
    };
    trace!(bio_len = biography.len(), "parsed biography");

    debug!(
        %name, %nickname, %rating, %dpr, %psg,
        "parsed mercenary record"
    );

    Ok(Mercenary {
        name,
        nickname,
        age,
        height_feet,
        height_inches,
        weight,
        nation,
        rating,
        dpr,
        psg,
        avail,
        exp,
        str_stat,
        agl,
        wil,
        wsk,
        hhc,
        tch,
        enc,
        aps,
        fee_hire,
        fee_bonus,
        fee_death,
        mail,
        biography,
    })
}

/// Parse `MERCS.DAT` from the given file path, returning all mercenary records.
///
/// The file uses CR/LF line endings and `<` as record delimiters.
/// Trailing `~` sentinel lines are ignored.
///
/// # Errors
///
/// Returns [`MercsError::Io`] on file read failure, [`MercsError::Parse`] on
/// malformed records (with line numbers), or [`MercsError::Validation`] for
/// semantic issues.
pub fn parse_mercs(path: &Path) -> Result<Vec<Mercenary>, MercsError> {
    info!(path = %path.display(), "opening MERCS.DAT");
    let raw = std::fs::read_to_string(path)?;

    let lines: Vec<&str> = raw.lines().map(strip_cr).collect();

    // Split into records on `<` delimiter lines
    let mut records: Vec<Vec<(usize, &str)>> = Vec::new();
    let mut current: Vec<(usize, &str)> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Skip EOF sentinel lines
        if trimmed == "~" {
            continue;
        }
        if trimmed == "<" {
            if !current.is_empty() {
                records.push(current);
                current = Vec::new();
            }
            continue;
        }
        // 1-based line numbers
        current.push((i + 1, line));
    }
    // Handle any trailing record without a final `<`
    if !current.is_empty() {
        records.push(current);
    }

    info!(record_count = records.len(), "split file into records");

    let mut mercs = Vec::with_capacity(records.len());
    for record_lines in &records {
        if record_lines.is_empty() {
            continue;
        }
        let base_line = record_lines[0].0;
        let plain_lines: Vec<&str> = record_lines.iter().map(|(_, l)| *l).collect();
        let merc = parse_record(&plain_lines, base_line)?;
        mercs.push(merc);
    }

    info!(
        merc_count = mercs.len(),
        "successfully parsed all mercenaries"
    );
    Ok(mercs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthesized multi-record MERCS.DAT content (not from game data).
    const SAMPLE_DATA: &str = "\
Name:  John_Q._Doe\r\n\
Nickname:  Johnny\r\n\
Age:  30\tHgt:  6 0\tWgt:  185 lbs.\r\n\
Nation:  USA\r\n\
\r\n\
Missions:\tMissions Completed:\r\n\
\r\n\
RATING:  50             DPR:  130       PSG:  100         AVAIL: 1\r\n\
\r\n\
EXP:  40  STR:  55  AGL:  60\r\n\
WIL:  45  WSK:  50  HHC:  48\r\n\
TCH:  30  ENC:  300  APS:  38\r\n\
\r\n\
Fees:  80000\t35000\t150000  \r\n\
mail: 1\r\n\
\r\n\
A former Army Ranger who left service after a decade to pursue private military contracting.\r\n\
<\r\n\
Name:  Ana_Maria_Lopez\r\n\
Nickname:  Ana\r\n\
Age:  25\tHgt:  5 4\tWgt:  120 lbs.\r\n\
Nation:  South_America\r\n\
\r\n\
Missions:\tMissions Completed:\r\n\
\r\n\
RATING:  35             DPR:  115       PSG:  -50         AVAIL: 0\r\n\
\r\n\
EXP:  20  STR:  28  AGL:  70\r\n\
WIL:  30  WSK:  40  HHC:  35\r\n\
TCH:  15  ENC:  225  APS:  34\r\n\
\r\n\
Fees:  45000\t20000\t85000  \r\n\
mail: 0\r\n\
\r\n\
Grew up in the slums and learned to fight before she could read.\r\n\
<\r\n\
~\r\n\
~\r\n\
~\r\n";

    #[test]
    fn parse_multi_record() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_mercs.dat");
        std::fs::write(&path, SAMPLE_DATA).unwrap();

        let mercs = parse_mercs(&path).expect("parsing should succeed");
        assert_eq!(mercs.len(), 2);

        let john = &mercs[0];
        assert_eq!(john.name, "John Q. Doe");
        assert_eq!(john.nickname, "Johnny");
        assert_eq!(john.age, 30);
        assert_eq!(john.height_feet, 6);
        assert_eq!(john.height_inches, 0);
        assert_eq!(john.weight, 185);
        assert_eq!(john.nation, "USA");
        assert_eq!(john.rating, 50);
        assert_eq!(john.dpr, 130);
        assert_eq!(john.psg, 100);
        assert_eq!(john.avail, 1);
        assert_eq!(john.exp, 40);
        assert_eq!(john.str_stat, 55);
        assert_eq!(john.agl, 60);
        assert_eq!(john.wil, 45);
        assert_eq!(john.wsk, 50);
        assert_eq!(john.hhc, 48);
        assert_eq!(john.tch, 30);
        assert_eq!(john.enc, 300);
        assert_eq!(john.aps, 38);
        assert_eq!(john.fee_hire, 80000);
        assert_eq!(john.fee_bonus, 35000);
        assert_eq!(john.fee_death, 150000);
        assert_eq!(john.mail, 1);
        assert!(john.biography.contains("Army Ranger"));

        let ana = &mercs[1];
        assert_eq!(ana.name, "Ana Maria Lopez");
        assert_eq!(ana.nation, "South America");
        assert_eq!(ana.psg, -50);
        assert_eq!(ana.avail, 0);
        assert_eq!(ana.mail, 0);
        assert_eq!(ana.enc, 225);
    }

    #[test]
    fn underscore_to_space_conversion() {
        assert_eq!(underscores_to_spaces("Maria_Hernandez"), "Maria Hernandez");
        assert_eq!(underscores_to_spaces("South_Korea"), "South Korea");
        assert_eq!(underscores_to_spaces("NOT_AVAILABLE"), "NOT AVAILABLE");
        assert_eq!(underscores_to_spaces("NoUnderscores"), "NoUnderscores");
        assert_eq!(underscores_to_spaces("A_B_C_D"), "A B C D");
    }

    #[test]
    fn edge_case_negative_psg_and_minimal_stats() {
        let data = "\
Name:  Test_Merc\r\n\
Nickname:  Testy\r\n\
Age:  19\tHgt:  5 2\tWgt:  110 lbs.\r\n\
Nation:  Nowhere\r\n\
\r\n\
Missions:\tMissions Completed:\r\n\
\r\n\
RATING:  10             DPR:  112       PSG:  -160         AVAIL: 1\r\n\
\r\n\
EXP:  05  STR:  12  AGL:  06\r\n\
WIL:  03  WSK:  09  HHC:  14\r\n\
TCH:  05  ENC:  225  APS:  30\r\n\
\r\n\
Fees:  21000\t9000\t39500  \r\n\
mail: 0\r\n\
\r\n\
No combat experience whatsoever.\r\n\
<\r\n\
~\r\n";

        let dir = std::env::temp_dir();
        let path = dir.join("test_mercs_edge.dat");
        std::fs::write(&path, data).unwrap();

        let mercs = parse_mercs(&path).expect("parsing should succeed");
        assert_eq!(mercs.len(), 1);

        let m = &mercs[0];
        assert_eq!(m.name, "Test Merc");
        assert_eq!(m.age, 19);
        assert_eq!(m.height_feet, 5);
        assert_eq!(m.height_inches, 2);
        assert_eq!(m.weight, 110);
        assert_eq!(m.psg, -160);
        assert_eq!(m.exp, 5);
        assert_eq!(m.str_stat, 12);
        assert_eq!(m.agl, 6);
        assert_eq!(m.wil, 3);
        assert_eq!(m.wsk, 9);
        assert_eq!(m.hhc, 14);
        assert_eq!(m.tch, 5);
        assert_eq!(m.enc, 225);
        assert_eq!(m.aps, 30);
        assert_eq!(m.fee_hire, 21000);
        assert_eq!(m.fee_bonus, 9000);
        assert_eq!(m.fee_death, 39500);
    }

    #[test]
    fn strip_cr_works() {
        assert_eq!(strip_cr("hello\r"), "hello");
        assert_eq!(strip_cr("hello"), "hello");
        assert_eq!(strip_cr(""), "");
    }

    #[test]
    fn missing_file_returns_io_error() {
        let result = parse_mercs(Path::new("/nonexistent/mercs.dat"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MercsError::Io(_)));
    }
}
