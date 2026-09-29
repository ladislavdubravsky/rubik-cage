//! Versioned, deterministic storage for exact evaluations. Legacy hash tables are rejected.
use super::{Evaluation, EvaluationMap, retrograde};
use crate::compat::{EvaluationV1, KeyV1};
use crate::core::position::PositionKey;
use bincode::{Decode, Encode};
use std::{fs, io::Write, path::Path};

const MAGIC: [u8; 8] = *b"RCGEVAL1";
/// v1: two players; simultaneous lines and infinite play draw; no immediate inverse.
pub const RULES_VERSION: u32 = crate::compat::SINGLE_COLOR_RULES;
const KEY_VERSION: u32 = 1;
const SOLVER_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Coverage {
    Complete,
    Subset,
}

#[derive(Debug)]
pub struct Table {
    pub roots: Vec<PositionKey>,
    pub coverage: Coverage,
    pub values: EvaluationMap,
}

#[derive(Encode, Decode)]
enum CoverageV1 {
    Complete,
    Subset,
}

#[derive(Encode, Decode)]
struct File {
    magic: [u8; 8],
    rules: u32,
    key_version: u32,
    solver_version: u32,
    /// Number of individual plies; terminal distance = 0.
    distance_version: u32,
    generator: String,
    roots: Vec<KeyV1>,
    coverage: CoverageV1,
    entries: Vec<(KeyV1, EvaluationV1)>,
}

impl Table {
    pub fn complete(root: PositionKey, values: EvaluationMap) -> Result<Self, String> {
        let table = Self {
            roots: vec![root],
            coverage: Coverage::Complete,
            values,
        };
        table.verify()?;
        Ok(table)
    }

    pub fn verify(&self) -> Result<(), String> {
        for root in &self.roots {
            root.validate().map_err(str::to_owned)?;
        }
        for (key, &value) in &self.values {
            key.validate().map_err(str::to_owned)?;
            if value.winner().is_some_and(|id| id > 1) {
                return Err("Invalid winner".into());
            }
            let terminal = Evaluation::terminal(&key.to_state());
            if terminal.is_some() && terminal != Some(value) {
                return Err("Invalid terminal value".into());
            }
            if terminal.is_none() && value.plies() == Some(0) {
                return Err("Nonterminal win in zero".into());
            }
        }
        if self.coverage == Coverage::Complete {
            if self.roots.iter().any(|r| !self.values.contains_key(r)) {
                return Err("Missing root".into());
            }
            retrograde::verify(&self.values)?;
        }
        Ok(())
    }

    /// Missing entries in a subset remain unknown, including omitted drawn states.
    pub fn filter(&mut self, minimum_plies: u32) {
        self.values
            .retain(|_, value| value.plies().is_some_and(|d| d >= minimum_plies));
        self.coverage = Coverage::Subset;
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.verify()?;
        let mut entries: Vec<_> = self
            .values
            .iter()
            .map(|(&k, &v)| Ok((KeyV1::from_current(k)?, EvaluationV1::from(v))))
            .collect::<Result<_, &str>>()
            .map_err(str::to_owned)?;
        entries.sort_by_key(|(k, _)| *k);
        let file = File {
            magic: MAGIC,
            rules: RULES_VERSION,
            key_version: KEY_VERSION,
            solver_version: SOLVER_VERSION,
            distance_version: 1,
            generator: concat!(
                "rubik-cage ",
                env!("CARGO_PKG_VERSION"),
                " verified-exact-v1"
            )
            .into(),
            roots: self
                .roots
                .iter()
                .copied()
                .map(KeyV1::from_current)
                .collect::<Result<_, _>>()
                .map_err(str::to_owned)?,
            coverage: match self.coverage {
                Coverage::Complete => CoverageV1::Complete,
                Coverage::Subset => CoverageV1::Subset,
            },
            entries,
        };
        bincode::encode_to_vec(file, bincode::config::standard()).map_err(|e| e.to_string())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if !bytes.starts_with(&MAGIC) {
            return Err(
                "Legacy or unsupported evaluation file; regenerate with the exact solver".into(),
            );
        }
        let (file, used): (File, usize) = bincode::decode_from_slice(
            bytes,
            bincode::config::standard().with_limit::<536870912>(),
        )
        .map_err(|e| e.to_string())?;
        if used != bytes.len()
            || file.rules != RULES_VERSION
            || file.key_version != KEY_VERSION
            || file.solver_version != SOLVER_VERSION
            || file.distance_version != 1
        {
            return Err("Incompatible evaluation format or trailing data".into());
        }
        let mut values = EvaluationMap::new();
        for (key, value) in file.entries {
            let key = key.into_current().map_err(str::to_owned)?;
            let value = value.into_current().map_err(str::to_owned)?;
            if values.insert(key, value).is_some() {
                return Err("Duplicate evaluation key".into());
            }
        }
        let table = Self {
            roots: file
                .roots
                .into_iter()
                .map(KeyV1::into_current)
                .collect::<Result<_, _>>()
                .map_err(str::to_owned)?,
            coverage: match file.coverage {
                CoverageV1::Complete => Coverage::Complete,
                CoverageV1::Subset => Coverage::Subset,
            },
            values,
        };
        table.verify()?;
        Ok(table)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        Self::decode(&fs::read(path).map_err(|e| e.to_string())?)
    }

    /// Validate first and rename only after the entire new file has been written.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let bytes = self.encode()?;
        let path = path.as_ref();
        let temporary = path.with_extension("bin.tmp");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::game::GameState,
        search::{
            Evaluation, merge_exact,
            retrograde::{Limits, solve},
        },
    };

    #[test]
    fn roundtrip_filter_and_format_rejection() {
        let root = GameState::new(3, 0);
        let values = solve(&root, &EvaluationMap::new(), Limits::default())
            .unwrap()
            .values;
        let mut table = Table::complete(root.position_key(), values).unwrap();
        let bytes = table.encode().unwrap();
        assert_eq!(Table::decode(&bytes).unwrap().values, table.values);
        assert_eq!(bytes, Table::decode(&bytes).unwrap().encode().unwrap());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(Table::decode(&trailing).is_err());
        assert!(Table::decode(include_bytes!("../../assets/eval.bin")).is_err());
        let (mut file, _): (File, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        file.rules += 1;
        assert!(
            Table::decode(&bincode::encode_to_vec(file, bincode::config::standard()).unwrap())
                .is_err()
        );
        table.filter(3);
        let filtered = Table::decode(&table.encode().unwrap()).unwrap();
        assert_eq!(filtered.coverage, Coverage::Subset);
        assert!(filtered.values.len() < Table::decode(&bytes).unwrap().values.len());
        assert!(filtered.values.values().all(|v| v.plies().unwrap() >= 3));
    }

    #[test]
    fn conflicting_merge_is_atomic() {
        let key = GameState::new(3, 0).position_key();
        let mut table = EvaluationMap::from([(
            key,
            Evaluation::Win {
                winner: 0,
                plies: 5,
            },
        )]);
        let before = table.clone();
        assert!(merge_exact(&mut table, EvaluationMap::from([(key, Evaluation::Draw)])).is_err());
        assert_eq!(table, before);
    }
}
