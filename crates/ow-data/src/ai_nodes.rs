//! Parser for `AINODEnn.DAT` and `NODESnn.DAT` — AI pathfinding node graphs.
//!
//! These files define waypoint graphs for AI pathfinding on mission maps.
//! Each node represents a navigable tile position with cardinal-direction
//! connections to other nodes. Both file families share the exact same format
//! but may contain different graph data for the same mission.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, trace, warn};

/// Errors that can occur while parsing AI node graph files.
#[derive(Debug, Error)]
pub enum AiNodesError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("line {line}: missing node count header")]
    MissingNodeCount { line: usize },

    #[error("line {line}: failed to parse node count: {detail}")]
    InvalidNodeCount { line: usize, detail: String },

    #[error("line {line}: expected 7 fields per node, got {found}")]
    InvalidFieldCount { line: usize, found: usize },

    #[error("line {line}: failed to parse field '{field}': {detail}")]
    InvalidField {
        line: usize,
        field: &'static str,
        detail: String,
    },

    #[error("declared {declared} nodes but parsed {parsed}")]
    NodeCountMismatch { declared: usize, parsed: usize },
}

/// A single waypoint node in the AI pathfinding graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiNode {
    /// Tile ID (linear index into the map's tile array).
    pub tile_id: u32,
    /// Grid quadrant/sub-position within the tile (1-4).
    pub grid: u8,
    /// Index of the neighbor node to the north, or -1 if none.
    pub north: i32,
    /// Index of the neighbor node to the east, or -1 if none.
    pub east: i32,
    /// Index of the neighbor node to the south, or -1 if none.
    pub south: i32,
    /// Index of the neighbor node to the west, or -1 if none.
    pub west: i32,
    /// Location flag: 0 = outdoor, 1 = building, 4 = special.
    pub inside: u8,
}

/// The complete AI pathfinding node graph for a mission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiNodeGraph {
    /// Total number of nodes declared in the file header.
    pub total_nodes: usize,
    /// The parsed node list, indexed 0..total_nodes-1.
    pub nodes: Vec<AiNode>,
}

/// Parse an `AINODEnn.DAT` or `NODESnn.DAT` file into an [`AiNodeGraph`].
pub fn parse_ai_nodes(path: &Path) -> Result<AiNodeGraph, AiNodesError> {
    info!("parsing AI node graph from {}", path.display());

    let contents = std::fs::read_to_string(path).map_err(|e| AiNodesError::Io {
        path: path.display().to_string(),
        source: e,
    })?;

    let mut total_nodes: Option<usize> = None;
    let mut nodes = Vec::new();

    for (line_idx, raw_line) in contents.lines().enumerate() {
        let line_num = line_idx + 1;
        let line = raw_line.trim_end_matches('\r').trim();

        // Skip blank lines.
        if line.is_empty() {
            continue;
        }

        // Skip the title comment (starts with #).
        if line.starts_with('#') {
            trace!("line {line_num}: header comment: {line}");
            continue;
        }

        // Skip comment lines (`;`).
        if line.starts_with(';') {
            trace!("line {line_num}: comment: {line}");
            continue;
        }

        // If we haven't parsed the node count yet, this line should be it.
        if total_nodes.is_none() {
            // Format: "<count> ; Total # Of Nodes In The File"
            let count_str = line.split(';').next().unwrap_or("").trim();
            let count: usize =
                count_str
                    .parse()
                    .map_err(|_| AiNodesError::InvalidNodeCount {
                        line: line_num,
                        detail: format!("'{count_str}' is not a valid integer"),
                    })?;
            info!("declared node count: {count}");
            total_nodes = Some(count);
            nodes.reserve(count);
            continue;
        }

        // Data line: 7 whitespace-separated integers.
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 7 {
            warn!(
                "line {line_num}: unexpected field count {} (expected 7), skipping: {line}",
                fields.len()
            );
            return Err(AiNodesError::InvalidFieldCount {
                line: line_num,
                found: fields.len(),
            });
        }

        let parse_field = |idx: usize, name: &'static str| -> Result<i64, AiNodesError> {
            fields[idx]
                .parse::<i64>()
                .map_err(|_| AiNodesError::InvalidField {
                    line: line_num,
                    field: name,
                    detail: format!("'{}' is not a valid integer", fields[idx]),
                })
        };

        let node = AiNode {
            tile_id: parse_field(0, "tile_id")? as u32,
            grid: parse_field(1, "grid")? as u8,
            north: parse_field(2, "north")? as i32,
            east: parse_field(3, "east")? as i32,
            south: parse_field(4, "south")? as i32,
            west: parse_field(5, "west")? as i32,
            inside: parse_field(6, "inside")? as u8,
        };

        let node_idx = nodes.len();
        debug!(
            "node {node_idx}: tile={} grid={} N={} E={} S={} W={} inside={}",
            node.tile_id, node.grid, node.north, node.east, node.south, node.west, node.inside
        );
        nodes.push(node);
    }

    let declared = total_nodes.ok_or(AiNodesError::MissingNodeCount { line: 0 })?;

    if nodes.len() != declared {
        warn!(
            "node count mismatch: declared {declared}, parsed {}",
            nodes.len()
        );
        return Err(AiNodesError::NodeCountMismatch {
            declared,
            parsed: nodes.len(),
        });
    }

    info!(
        "successfully parsed {} nodes from {}",
        nodes.len(),
        path.display()
    );

    Ok(AiNodeGraph {
        total_nodes: declared,
        nodes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(contents: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn parse_minimal_graph() {
        let data = "\
# AI NODE LIST -- MISSION #99\r\n\
\r\n\
3 ; Total # Of Nodes In The File\r\n\
\r\n\
;Tile\tGrid\tNorth\tEast\tSouth\tWest\tInside\r\n\
;-----------------------------------------------------\r\n\
\r\n\
 100\t1\t-1\t1\t-1\t-1\t0\r\n\
 200\t2\t-1\t-1\t2\t0\t1\r\n\
 300\t4\t1\t-1\t-1\t-1\t4\r\n\
";
        let f = write_temp_file(data);
        let graph = parse_ai_nodes(f.path()).unwrap();

        assert_eq!(graph.total_nodes, 3);
        assert_eq!(graph.nodes.len(), 3);

        assert_eq!(
            graph.nodes[0],
            AiNode {
                tile_id: 100,
                grid: 1,
                north: -1,
                east: 1,
                south: -1,
                west: -1,
                inside: 0,
            }
        );
        assert_eq!(graph.nodes[1].inside, 1);
        assert_eq!(graph.nodes[2].inside, 4);
    }

    #[test]
    fn progress_markers_skipped() {
        let data = "\
# AI NODE LIST -- MISSION #1\r\n\
\r\n\
11 ; Total # Of Nodes In The File\r\n\
\r\n\
;Tile\tGrid\tNorth\tEast\tSouth\tWest\tInside\r\n\
;-----------------------------------------------------\r\n\
\r\n\
 1\t1\t-1\t-1\t-1\t-1\t0\r\n\
 2\t1\t-1\t-1\t-1\t-1\t0\r\n\
 3\t1\t-1\t-1\t-1\t-1\t0\r\n\
 4\t1\t-1\t-1\t-1\t-1\t0\r\n\
 5\t1\t-1\t-1\t-1\t-1\t0\r\n\
 6\t1\t-1\t-1\t-1\t-1\t0\r\n\
 7\t1\t-1\t-1\t-1\t-1\t0\r\n\
 8\t1\t-1\t-1\t-1\t-1\t0\r\n\
 9\t1\t-1\t-1\t-1\t-1\t0\r\n\
 10\t1\t-1\t-1\t-1\t-1\t0\r\n\
; 10\t \r\n\
 11\t1\t-1\t-1\t-1\t-1\t0\r\n\
";
        let f = write_temp_file(data);
        let graph = parse_ai_nodes(f.path()).unwrap();
        assert_eq!(graph.total_nodes, 11);
        assert_eq!(graph.nodes.len(), 11);
    }

    #[test]
    fn node_count_mismatch_detected() {
        let data = "\
# AI NODE LIST -- MISSION #1\r\n\
\r\n\
5 ; Total # Of Nodes In The File\r\n\
\r\n\
 1\t1\t-1\t-1\t-1\t-1\t0\r\n\
 2\t1\t-1\t-1\t-1\t-1\t0\r\n\
";
        let f = write_temp_file(data);
        let err = parse_ai_nodes(f.path()).unwrap_err();
        assert!(matches!(
            err,
            AiNodesError::NodeCountMismatch {
                declared: 5,
                parsed: 2
            }
        ));
    }

    #[test]
    fn bad_field_count_rejected() {
        let data = "\
# test\r\n\
1 ; Total # Of Nodes In The File\r\n\
 100\t1\t-1\t-1\r\n\
";
        let f = write_temp_file(data);
        let err = parse_ai_nodes(f.path()).unwrap_err();
        assert!(matches!(err, AiNodesError::InvalidFieldCount { found: 4, .. }));
    }
}
