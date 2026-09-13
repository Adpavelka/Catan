use serde::{Deserialize, Serialize};
use rand::{seq::SliceRandom, SeedableRng};
use rand::rngs::StdRng;
use shared::ResourceType;
use std::path::Path;
use crate::{errors::GameError, game::entities::board::{Board, Coordinates}};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Robber {
    pos: Coordinates,
    asset_name: String,
}

impl Robber {
    fn assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("frontend")
            .join("assets")
            .join("robber")
    }

    fn available_assets_from_dir(dir: &Path) -> Vec<String> {
        let mut assets = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                let name = path.file_name()?.to_str()?;
                if !name.ends_with(".svg") {
                    return None;
                }

                let stem = path.file_stem()?.to_str()?;
                if stem == "robber" || stem.starts_with("robber_") {
                    Some(stem.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if assets.is_empty() {
            return vec!["robber".to_string()];
        }

        assets.sort();
        assets
    }

    pub fn available_assets() -> Vec<String> {
        Self::available_assets_from_dir(&Self::assets_dir())
    }

    pub fn new(board: &Board) -> Self {
        Self::new_for_seed(board, rand::random())
    }

    pub fn new_for_seed(board: &Board, seed: u64) -> Self {
        let asset_name = Self::choose_asset_for_seed(seed);

        for (coord, hex) in &board.hexes {
            if hex.resource == ResourceType::Desert {
                return Self {
                    pos: *coord,
                    asset_name,
                };
            }
        }

        panic!("No desert hex found on the board");
    }

    fn choose_asset_for_seed(seed: u64) -> String {
        let mut rng = StdRng::seed_from_u64(seed);
        Self::available_assets()
            .choose(&mut rng)
            .cloned()
            .unwrap_or("robber".to_string())
    }

    pub fn move_to(&mut self, coords: Coordinates) -> Result<(), GameError> {
        if coords == self.pos {
            return Err(GameError::InvalidRobberPosition);
        }

        self.pos = coords;
        Ok(())
    }

    pub fn get_pos(&self) -> Coordinates {
        self.pos
    }

    pub fn get_asset_name(&self) -> &str {
        &self.asset_name
    }
}

impl Default for Robber {
    fn default() -> Self {
        Self {
            pos: (0, 0),
            asset_name: "robber".to_string(),
        }
    }
}




#[cfg(test)]
mod tests {
    use crate::game::entities::board::Board;
    use crate::game::entities::robber::Robber;
    use crate::game::entities::resources::ResourceType;
    #[test]
    fn robber_new_starts_on_desert_hex() {
        let board = Board::new();
        let robber = Robber::new(&board);

        let hex = board
            .hexes
            .get(&robber.pos)
            .expect("robber position must exist in board hexes");
        assert_eq!(hex.resource, ResourceType::Desert);
    }

    #[test]
    fn robber_default_position_is_placeholder() {
        let r = Robber::default();
        assert_eq!(r.pos, (0, 0));
    }

    #[test]
    fn robber_chooses_a_valid_asset_name_for_the_game() {
        let board = Board::new();
        let robber = Robber::new_for_seed(&board, 42);
        let assets = Robber::available_assets();
        assert!(assets.iter().any(|asset| asset == robber.get_asset_name()));
    }

    #[test]
    fn robber_selection_uses_the_live_pool() {
        let board = Board::new();
        for seed in 0..1000u64 {
            let robber = Robber::new_for_seed(&board, seed);
            assert!(Robber::available_assets().iter().any(|asset| asset == robber.get_asset_name()));
        }
    }

    #[test]
    fn robber_default_is_the_plain_fallback() {
        let robber = Robber::default();
        assert_eq!(robber.get_asset_name(), "robber");
        assert!(Robber::available_assets().iter().any(|asset| asset == robber.get_asset_name()));
    }

    #[test]
    fn robber_uses_multiple_skins_across_game_seeds() {
        let board = Board::new();
        let unique_assets: std::collections::HashSet<_> = (0..200u64)
            .map(|seed| Robber::new_for_seed(&board, seed).get_asset_name().to_string())
            .collect();

        assert!(unique_assets.len() > 1, "seeded robber selection collapsed to one skin: {:?}", unique_assets);
    }

    #[test]
    fn robber_auto_discovers_assets_from_the_folder() {
        let dir = std::env::temp_dir().join("colonist-robber-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("robber.svg"), "").unwrap();
        std::fs::write(dir.join("robber_fox.svg"), "").unwrap();
        std::fs::write(dir.join("robber_raccoon.svg"), "").unwrap();
        std::fs::write(dir.join("other.svg"), "").unwrap();

        let assets = Robber::available_assets_from_dir(&dir);
        assert!(assets.contains(&"robber".to_string()));
        assert!(assets.contains(&"robber_fox".to_string()));
        assert!(assets.contains(&"robber_raccoon".to_string()));
        assert!(!assets.contains(&"other".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }

}
