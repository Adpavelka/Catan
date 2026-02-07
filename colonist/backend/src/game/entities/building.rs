use crate::game::entities::resources::ResourceSet;
use shared::ResourceType;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VertexBuilding {
    Settlement,
    City,
}

impl VertexBuilding {
    pub fn cost(&self) -> ResourceSet {
        let mut cost = ResourceSet::new();
        match self {
            Self::Settlement => {
                cost.add(ResourceType::Wood, 1);
                cost.add(ResourceType::Brick, 1);
                cost.add(ResourceType::Sheep, 1);
                cost.add(ResourceType::Wheat, 1);
            }
            Self::City => {
                cost.add(ResourceType::Ore, 3);
                cost.add(ResourceType::Wheat, 2);
            }
        }
        cost
    }

    pub fn victory_points(&self) -> u8 {
        match self {
            Self::Settlement => 1,
            Self::City => 2,
        }
    }

    pub fn production(&self) -> u32 {
        match self {
            Self::Settlement => 1,
            Self::City => 2,
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EdgeBuilding {
    Road,
}

impl EdgeBuilding {
    pub fn cost(&self) -> ResourceSet {
        let mut cost = ResourceSet::new();
        cost.add(ResourceType::Wood, 1);
        cost.add(ResourceType::Brick, 1);
        cost
    }

    pub fn victory_points(&self) -> u8 {
        0
    }
}




#[cfg(test)]
mod tests {
    use crate::game::entities::building::VertexBuilding::{City, Settlement};
    use crate::game::entities::building::EdgeBuilding::{Road};
    use shared::{ResourceType};
    
    #[test]
    fn settlement_cost_vp_glyph_and_production() {
        let s = Settlement;

        let cost = s.cost();
        assert_eq!(cost.amount_of(ResourceType::Wood), 1);
        assert_eq!(cost.amount_of(ResourceType::Brick), 1);
        assert_eq!(cost.amount_of(ResourceType::Sheep), 1);
        assert_eq!(cost.amount_of(ResourceType::Wheat), 1);
        assert_eq!(cost.amount_of(ResourceType::Ore), 0);

        assert_eq!(s.victory_points(), 1);
        assert_eq!(s.production(), 1);
    }

    #[test]
    fn city_cost_vp_glyph_and_production() {
        let c = City;

        let cost = c.cost();
        assert_eq!(cost.amount_of(ResourceType::Ore), 3);
        assert_eq!(cost.amount_of(ResourceType::Wheat), 2);
        assert_eq!(cost.amount_of(ResourceType::Wood), 0);
        assert_eq!(cost.amount_of(ResourceType::Brick), 0);
        assert_eq!(cost.amount_of(ResourceType::Sheep), 0);

        assert_eq!(c.victory_points(), 2);
        assert_eq!(c.production(), 2);
    }

    #[test]
    fn road_cost_vp_and_glyph() {
        let r = Road;

        let cost = r.cost();
        assert_eq!(cost.amount_of(ResourceType::Wood), 1);
        assert_eq!(cost.amount_of(ResourceType::Brick), 1);
        assert_eq!(cost.amount_of(ResourceType::Sheep), 0);
        assert_eq!(cost.amount_of(ResourceType::Wheat), 0);
        assert_eq!(cost.amount_of(ResourceType::Ore), 0);

        assert_eq!(r.victory_points(), 0);
    }

    #[test]
    fn settlement_produces_one_resource() {
        let vb = Settlement;
        assert_eq!(vb.production(), 1);
    }

    #[test]
    fn city_produces_two_resources() {
        let vb = City;
        assert_eq!(vb.production(), 2);
    }

    #[test]
    fn settlement_gives_one_victory_point() {
        let vb = Settlement;
        assert_eq!(vb.victory_points(), 1);
    }

    #[test]
    fn city_gives_two_victory_points() {
        let vb = City;
        assert_eq!(vb.victory_points(), 2);
    }
}