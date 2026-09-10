use crate::game::entities::building::{EdgeBuilding, VertexBuilding};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use shared::{BoardInfo, BoardLayout, BuildingInfo, HexInfo, PortInfo, ResourceType};
use std::collections::{HashMap, HashSet};
use std::iter::repeat_n;
use uuid::Uuid;

pub type Coordinates = (i32, i32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hex {
    pub coord: Coordinates,
    pub resource: ResourceType,
    pub number: u8,
    pub adjacent_vertices: [Coordinates; 6],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub coord: Coordinates,
    pub building: Option<VertexBuilding>,
    pub owner: Option<Uuid>,
    pub adjacent_edges: HashSet<Coordinates>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub coord: Coordinates,
    pub building: Option<EdgeBuilding>,
    pub owner: Option<Uuid>,
    pub adjacent_vertices: (Coordinates, Coordinates),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PortType {
    ThreeToOne,
    TwoToOne(ResourceType),
}

impl From<&PortType> for shared::PortType {
    fn from(p: &PortType) -> Self {
        match p {
            PortType::ThreeToOne => shared::PortType::ThreeToOne,
            PortType::TwoToOne(res) => shared::PortType::TwoToOne(*res),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub coord: [Coordinates; 2],
    pub port_type: PortType,
}

impl From<&Port> for PortInfo {
    fn from(p: &Port) -> Self {
        PortInfo {
            vertices: [
                (p.coord[0].0, p.coord[0].1),
                (p.coord[1].0, p.coord[1].1),
            ],
            port_type: (&p.port_type).into(),
        }
    }
}
#[serde_as]
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Board {
    #[serde_as(as = "Vec<(_, _)>")]
    pub hexes: HashMap<Coordinates, Hex>,
    #[serde_as(as = "Vec<(_, _)>")]
    pub vertices: HashMap<Coordinates, Vertex>,
    #[serde_as(as = "Vec<(_, _)>")]
    pub edges: HashMap<Coordinates, Edge>,
    #[serde_as(as = "Vec<(_, _)>")]
    pub ports: HashMap<Coordinates, Port>,
}

impl Board {
    pub fn new() -> Self {
        Self::new_standard_board()
    }

    fn get_hex_neighbours() -> [Coordinates; 6] {
        [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)]
    }

    pub fn is_edge_touching_vertex(&self, edge_coord: Coordinates, vertex_coord: Coordinates) -> bool {
        match self.edges.get(&edge_coord) {
            Some(edge) => {
                let (a, b) = edge.adjacent_vertices;
                a == vertex_coord || b == vertex_coord
            }

            None => false,
        }
    }

    pub fn get_adjacent_hexes(hex_coord: Coordinates) -> [Coordinates; 6] {
        let (q, r) = hex_coord;
        let base = (q * 3, r * 3);
        let neighbours = Self::get_hex_neighbours();
        let mut vertices = [base; 6];

        for i in 0..6 {
            vertices[i] = (
                base.0 + neighbours[i].0 + neighbours[(i + 1) % 6].0,
                base.1 + neighbours[i].1 + neighbours[(i + 1) % 6].1,
            );
        }

        vertices
    }

    fn edge_key(a: Coordinates, b: Coordinates) -> Coordinates {
        ((a.0 + b.0) / 3, (a.1 + b.1) / 3)
    }

    /// The hexes that make up a layout.
    ///
    /// `Standard` is the base game's radius-2 hexagon (19 hexes). `Extended`
    /// is the 5-6 player board: rows of 3-4-5-6-5-4-3, i.e. 30 hexes.
    pub fn hex_coords_for(layout: BoardLayout) -> Vec<Coordinates> {
        let radius = match layout {
            BoardLayout::Standard => 2,
            BoardLayout::Extended => 3,
        };

        let mut coords = Vec::new();
        for r in -radius..=radius {
            let q_min = (-radius).max(-r - radius);
            let q_max = radius.min(-r + radius);

            for q in q_min..=q_max {
                coords.push((q, r));
            }
        }

        if layout == BoardLayout::Extended {
            // A radius-3 hexagon is 37 hexes; the extension board is 30. Drop
            // one hex from the end of each row to reach 3-4-5-6-5-4-3.
            let mut trimmed: Vec<Coordinates> = Vec::new();
            for r in -radius..=radius {
                let mut row: Vec<Coordinates> =
                    coords.iter().copied().filter(|(_, rr)| *rr == r).collect();
                row.sort();
                // Alternate which end is trimmed. Taking from the same side
                // every time shears the whole board; alternating leaves the
                // half-hex row offset a hex grid has anyway.
                if r.rem_euclid(2) == 0 { row.pop(); } else { row.remove(0); }
                trimmed.extend(row);
            }
            return trimmed;
        }

        coords
    }

    fn generate_from_layout(layout: BoardLayout, resources: Vec<ResourceType>, numbers: Vec<u8>) -> Self {
        let mut board = Board::default();

        let hex_coords = Self::hex_coords_for(layout);

        let mut res_iter = resources.into_iter();
        let mut num_iter = numbers.into_iter();

        for &hex_coord in &hex_coords {
            let resource = res_iter.next().unwrap_or(ResourceType::Desert);
            let number = num_iter.next().unwrap_or(0);

            board.hexes.insert(
                hex_coord,
                Hex {
                    coord: hex_coord,
                    resource,
                    number,
                    adjacent_vertices: Self::get_adjacent_hexes(hex_coord),
                },
            );
        }


        for &hex_coord in &hex_coords {
            let vertices = Self::get_adjacent_hexes(hex_coord);

            for i in 0..6 {
                let a = vertices[i];
                let b = vertices[(i + 1) % 6];
                let edge_coord = Self::edge_key(a, b);

                board.edges.entry(edge_coord).or_insert_with(|| Edge {
                    coord: edge_coord,
                    building: None,
                    owner: None,
                    adjacent_vertices: (a, b),
                });

                board.vertices
                    .entry(a)
                    .or_insert_with(|| Vertex {
                        coord: a,
                        building: None,
                        owner: None,
                        adjacent_edges: HashSet::new(),
                    })
                    .adjacent_edges
                    .insert(edge_coord);

                board.vertices
                    .entry(b)
                    .or_insert_with(|| Vertex {
                        coord: b,
                        building: None,
                        owner: None,
                        adjacent_edges: HashSet::new(),
                    })
                    .adjacent_edges
                    .insert(edge_coord);
            }
        }

        board
    }


    /// The terrain and number tokens for a layout, in a fixed order. Callers
    /// shuffle; this only fixes the *counts*.
    ///
    /// Standard is the base game. Extended follows the 5-6 player extension:
    /// 30 hexes with two deserts, and 28 tokens rather than 18.
    fn layout_pieces(layout: BoardLayout) -> (Vec<ResourceType>, Vec<u8>) {
        use ResourceType::*;

        match layout {
            BoardLayout::Standard => {
                let mut resources = Vec::with_capacity(19);
                resources.extend(repeat_n(Wood, 4));
                resources.extend(repeat_n(Sheep, 4));
                resources.extend(repeat_n(Wheat, 4));
                resources.extend(repeat_n(Brick, 3));
                resources.extend(repeat_n(Ore, 3));
                resources.push(Desert);

                let numbers = vec![
                    2, 3, 3, 4, 4, 5, 5, 6, 6, 8, 8, 9, 9, 10, 10, 11, 11, 12,
                    0,
                ];
                (resources, numbers)
            }

            BoardLayout::Extended => {
                let mut resources = Vec::with_capacity(30);
                resources.extend(repeat_n(Wood, 6));
                resources.extend(repeat_n(Sheep, 6));
                resources.extend(repeat_n(Wheat, 6));
                resources.extend(repeat_n(Brick, 5));
                resources.extend(repeat_n(Ore, 5));
                resources.extend(repeat_n(Desert, 2));

                let mut numbers = vec![2, 2, 12, 12];
                for n in [3, 4, 5, 6, 8, 9, 10, 11] {
                    numbers.extend(repeat_n(n, 3));
                }
                numbers.extend(repeat_n(0, 2));
                (resources, numbers)
            }
        }
    }

    #[cfg(test)]
    fn test_layout() -> (Vec<ResourceType>, Vec<u8>) {
        Self::layout_pieces(BoardLayout::Standard)
    }

    fn random_layout(layout: BoardLayout) -> (Vec<ResourceType>, Vec<u8>) {
        use rand::seq::SliceRandom;
        use rand::{thread_rng, Rng};

        let (resources, numbers) = Self::layout_pieces(layout);
        let resource_total = resources.len();
        let mut rng = thread_rng();

        let mut land: Vec<ResourceType> = resources
            .into_iter()
            .filter(|resource| *resource != ResourceType::Desert)
            .collect();
        let mut tokens: Vec<u8> = numbers.into_iter().filter(|number| *number != 0).collect();

        land.shuffle(&mut rng);
        tokens.shuffle(&mut rng);

        // Put each desert back at a random slot, taking its blank token with it.
        let desert_count = resource_total - land.len();
        for _ in 0..desert_count {
            let slot = rng.gen_range(0..=land.len());
            land.insert(slot, ResourceType::Desert);
            tokens.insert(slot, 0);
        }

        (land, tokens)
    }


    /// Places `port_count` harbours evenly around the coast.
    ///
    /// The coast is derived from the board rather than hardcoded, so this works
    /// for any layout: a coastal edge is simply one that belongs to exactly a
    /// single hex. Harbours are then spread evenly by angle around the centre.
    fn add_ports(&mut self, port_count: usize) {
        use PortType::*;

        let mut edge_hex_count: HashMap<Coordinates, usize> = HashMap::new();
        for hex_coord in self.hexes.keys() {
            let corners = Self::get_adjacent_hexes(*hex_coord);
            for i in 0..6 {
                let key = Self::edge_key(corners[i], corners[(i + 1) % 6]);
                *edge_hex_count.entry(key).or_insert(0) += 1;
            }
        }

        let mut coastline: Vec<Coordinates> = edge_hex_count
            .into_iter()
            .filter(|(_, hexes)| *hexes == 1)
            .map(|(edge, _)| edge)
            .collect();

        // Order the coast so evenly spaced picks really are spread out.
        coastline.sort_by(|a, b| {
            Self::edge_angle(self, *a)
                .partial_cmp(&Self::edge_angle(self, *b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if coastline.is_empty() {
            return;
        }

        let mut port_types: Vec<PortType> = vec![
            TwoToOne(ResourceType::Brick),
            TwoToOne(ResourceType::Wood),
            TwoToOne(ResourceType::Sheep),
            TwoToOne(ResourceType::Wheat),
            TwoToOne(ResourceType::Ore),
        ];
        while port_types.len() < port_count {
            port_types.push(ThreeToOne);
        }
        port_types.truncate(port_count);

        let mut rng = rand::thread_rng();
        port_types.shuffle(&mut rng);

        let mut ports_map = HashMap::new();
        for (i, port_type) in port_types.into_iter().enumerate() {
            let edge_coord = coastline[i * coastline.len() / port_count];
            let Some(edge) = self.edges.get(&edge_coord) else { continue };

            let (a, b) = edge.adjacent_vertices;
            let port = Port { coord: [a, b], port_type };

            ports_map.insert(a, port.clone());
            ports_map.insert(b, port);
        }

        self.ports = ports_map;
    }

    /// Angle of an edge's midpoint about the board centre, for ordering the coast.
    fn edge_angle(&self, edge_coord: Coordinates) -> f64 {
        let Some(edge) = self.edges.get(&edge_coord) else { return 0.0 };
        let (a, b) = edge.adjacent_vertices;

        let point = |v: Coordinates| {
            let (q, r) = (v.0 as f64 / 3.0, v.1 as f64 / 3.0);
            (3f64.sqrt() * q + 3f64.sqrt() / 2.0 * r, 1.5 * r)
        };

        let (ax, ay) = point(a);
        let (bx, by) = point(b);
        ((ay + by) / 2.0).atan2((ax + bx) / 2.0)
    }

    pub fn unique_ports(&self) -> Vec<Port> {
        use std::collections::HashSet;

        let mut seen: HashSet<[Coordinates; 2]> = HashSet::new();
        let mut result: Vec<Port> = Vec::new();

        for port in self.ports.values() {
            let a = port.coord[0];
            let b = port.coord[1];
            let key = if a <= b { [a, b] } else { [b, a] };

            if seen.insert(key) {
                result.push(port.clone());
            }
        }

        result
    }

    fn new_standard_board() -> Self {
        Self::new_for_layout(BoardLayout::Standard, 9)
    }

    pub fn new_for_layout(layout: BoardLayout, port_count: usize) -> Self {
        let (resources, numbers) = Self::random_layout(layout);
        let mut board = Self::generate_from_layout(layout, resources, numbers);
        board.add_ports(port_count);
        board
    }

    pub fn is_buildable_vertex(&self, pos: Coordinates) -> bool {
        let Some(vertex) = self.vertices.get(&pos) else {
            return false;
        };

        if vertex.building.is_some() {
            return false;
        }

        for edge_coord in &vertex.adjacent_edges {
            let Some(edge) = self.edges.get(edge_coord) else {
                continue;
            };

            let (a, b) = edge.adjacent_vertices;
            let neighbor = if a == pos { b } else { a };

            if let Some(v) = self.vertices.get(&neighbor) {
                if v.building.is_some() {
                    return false;
                }
            }
        }

        true
    }


    pub fn is_vertex_connected_to_player(&self, pos: Coordinates, player_id: Uuid) -> bool {
        self.vertices
            .get(&pos)
            .map(|v| {
                v.adjacent_edges.iter().any(|&ek| {
                    self.edges.get(&ek).map(|e| {
                        match e.owner {
                            Some(id) => id == player_id,
                            None => false,
                        }
                    }).unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    pub fn is_edge_connected_to_player(&self, pos: Coordinates, player_id: Uuid) -> bool {
        let Some(edge) = self.edges.get(&pos) else {
            return false;
        };

        let vertices = [edge.adjacent_vertices.0, edge.adjacent_vertices.1];

        vertices.iter().any(|&vcoord| {
            let Some(v) = self.vertices.get(&vcoord) else {
                return false;
            };

            // Your own building at the junction always connects.
            if v.owner == Some(player_id) {
                return true;
            }

            // An opponent's settlement or city cuts the network at this
            // junction, so roads may not be continued through it.
            if v.building.is_some() {
                return false;
            }

            v.adjacent_edges.iter().any(|ek| {
                self.edges.get(ek).and_then(|e| e.owner) == Some(player_id)
            })
        })
    }

    pub fn build_vertex(&mut self, player_id: Uuid, pos: Coordinates, building: VertexBuilding) {
        if let Some(vertex) = self.vertices.get_mut(&pos) {
            vertex.building = Some(building);
            vertex.owner = Some(player_id);
        }
    }

    pub fn build_edge(&mut self, player_id: Uuid, pos: Coordinates, building: EdgeBuilding) {
        if let Some(edge) = self.edges.get_mut(&pos) {
            edge.building = Some(building);
            edge.owner = Some(player_id);
        }
    }

    /// Take a departed player's pieces off the board.
    ///
    /// Left standing they belong to nobody: they pay out to no one on a roll,
    /// they hold vertices against the distance rule so nobody may ever build
    /// there, and their settlements go on cutting other players' roads in two
    /// on behalf of someone who is no longer in the game.
    ///
    /// Returns how many vertex buildings and how many roads were cleared.
    pub fn remove_player_pieces(&mut self, player_id: Uuid) -> (usize, usize) {
        let mut buildings = 0;
        for vertex in self.vertices.values_mut() {
            if vertex.owner == Some(player_id) {
                vertex.building = None;
                vertex.owner = None;
                buildings += 1;
            }
        }

        let mut roads = 0;
        for edge in self.edges.values_mut() {
            if edge.owner == Some(player_id) {
                edge.building = None;
                edge.owner = None;
                roads += 1;
            }
        }

        (buildings, roads)
    }

    pub fn calculate_longest_road(&self, player_id: Uuid) -> usize {
        let player_edges: HashSet<_> = self
            .edges
            .iter()
            .filter(|(_, e)| e.owner == Some(player_id))
            .map(|(c, _)| *c)
            .collect();

        fn dfs(board: &Board, current_vertex: Coordinates, player_id: Uuid, player_edges: &HashSet<Coordinates>, visited_edges: &mut HashSet<Coordinates>) -> usize {
            let mut max_len = 0;

            if let Some(vertex) = board.vertices.get(&current_vertex) {
                for &edge_coord in &vertex.adjacent_edges {
                    if !player_edges.contains(&edge_coord) || visited_edges.contains(&edge_coord) {
                        continue;
                    }

                    let edge = &board.edges[&edge_coord];
                    let next_vertex = if edge.adjacent_vertices.0 == current_vertex {
                        edge.adjacent_vertices.1
                    } else {
                        edge.adjacent_vertices.0
                    };

                    if let Some(v) = board.vertices.get(&next_vertex) {
                        if let Some(owner) = v.owner {
                            if owner != player_id {
                                continue;
                            }
                        }
                    }

                    visited_edges.insert(edge_coord);
                    let len =
                        1 + dfs(board, next_vertex, player_id, player_edges, visited_edges);
                    visited_edges.remove(&edge_coord);

                    max_len = max_len.max(len);
                }
            }

            max_len
        }

        let mut max_length = 0;

        let all_vertices: HashSet<_> = player_edges
            .iter()
            .flat_map(|e| {
                let edge = &self.edges[e];
                [edge.adjacent_vertices.0, edge.adjacent_vertices.1]
            })
            .collect();

        for &vertex in &all_vertices {
            let mut visited_edges = HashSet::new();
            let length = dfs(self, vertex, player_id, &player_edges, &mut visited_edges);
            max_length = max_length.max(length);
        }

        max_length
    }

    pub fn to_info(&self, robber_pos: (i32, i32)) -> BoardInfo {
        let hexes = self.hexes.iter()
            .map(|(coords, hex)| HexInfo {
                q: coords.0,
                r: coords.1,
                resource: hex.resource,
                number: hex.number,
            })
            .collect();

        let mut settlements = Vec::new();
        let mut cities = Vec::new();

        for (coords, vertex) in &self.vertices {
            if let Some(building) = &vertex.building {
                let info = BuildingInfo {
                    player_id: vertex.owner.unwrap_or(Uuid::nil()),
                    x: coords.0,
                    y: coords.1,
                };

                match building {
                    VertexBuilding::Settlement => settlements.push(info),
                    VertexBuilding::City => cities.push(info),
                }
            }
        }

        let roads = self.edges.iter()
            .filter_map(|(coords, edge)| {
                edge.owner.map(|pid| BuildingInfo {
                    player_id: pid,
                    x: coords.0,
                    y: coords.1,
                })
            })
            .collect();

        let ports = self.unique_ports().into_iter()
            .map(|port| {
                PortInfo {
                    vertices: port.coord,
                    port_type: match port.port_type {
                        PortType::ThreeToOne => shared::PortType::ThreeToOne,
                        PortType::TwoToOne(res) => shared::PortType::TwoToOne(res),
                    },
                }
            })
            .collect();

        BoardInfo {
            hexes,
            settlements,
            cities,
            roads,
            robber_pos,
            ports,
        }
    }
}







#[cfg(test)]
mod tests {
    use shared::BoardLayout;
    use uuid::Uuid;

    use crate::game::entities::board::Board;
    use crate::game::entities::building::{VertexBuilding, EdgeBuilding};
    use crate::game::entities::board::Coordinates;
    use crate::game::entities::board::PortType;
    use crate::game::entities::resources::{ResourceType};

    fn pick_any_hex(board: &Board) -> Coordinates {
        *board.hexes.keys().min().expect("board must have hexes")
    }

    fn pick_any_vertex(board: &Board) -> Coordinates {
        *board.vertices.keys().min().expect("board must have vertices")
    }

    fn pick_any_edge(board: &Board) -> Coordinates {
        *board.edges.keys().min().expect("board must have edges")
    }

    /// The vertices one edge away from `v`. Note this is *not*
    /// `get_adjacent_hexes(v)`: that maps a **hex** coordinate to its corners,
    /// and feeding it a vertex coordinate yields unrelated points.
    fn vertex_neighbours(board: &Board, v: Coordinates) -> Vec<Coordinates> {
        let Some(vertex) = board.vertices.get(&v) else {
            return Vec::new();
        };

        vertex
            .adjacent_edges
            .iter()
            .filter_map(|e| board.edges.get(e))
            .map(|edge| {
                let (a, b) = edge.adjacent_vertices;
                if a == v { b } else { a }
            })
            .filter(|n| board.vertices.contains_key(n))
            .collect()
    }

    fn pick_buildable_vertex(board: &Board) -> Coordinates {
        *board
            .vertices
            .keys()
            .find(|&&v| board.is_buildable_vertex(v))
            .expect("expected at least one buildable vertex on an empty board")
    }

    #[test]
    fn test_board_creation() {
        let board = Board::new_standard_board();
        assert!(!board.hexes.is_empty(), "Board should have hexes");
        assert!(!board.vertices.is_empty(), "Board should have vertices");
        assert!(!board.edges.is_empty(), "Board should have edges");
        assert!(!board.ports.is_empty(), "Board should have ports");
    }

    #[test]
    fn test_layout_returns_19_resources_and_numbers_with_desert_and_zero() {
        let (resources, numbers) = Board::test_layout();
        assert_eq!(resources.len(), 19);
        assert_eq!(numbers.len(), 19);

        let desert_indices: Vec<usize> = resources
            .iter()
            .enumerate()
            .filter(|(_, r)| **r == ResourceType::Desert)
            .map(|(i, _)| i)
            .collect();

        assert_eq!(desert_indices.len(), 1, "There must be exactly one desert");

        let desert_index = desert_indices[0];
        assert_eq!(numbers[desert_index], 0, "Desert must have number 0");
    }

    #[test]
    fn get_adjacent_hexes_returns_six_unique_coords() {
        let board = Board::new_standard_board();
        let h = pick_any_hex(&board);

        let vs = Board::get_adjacent_hexes(h);
        assert_eq!(vs.len(), 6);

        let mut set = std::collections::HashSet::new();
        for v in vs {
            assert!(set.insert(v), "adjacent vertices must be unique");
        }
    }

    #[test]
    fn edge_key_matches_generated_edges() {
        let board = Board::new_standard_board();
        let e_coord = pick_any_edge(&board);
        let e = board.edges.get(&e_coord).unwrap();
        let (a, b) = e.adjacent_vertices;

        let k1 = Board::edge_key(a, b);
        let k2 = Board::edge_key(b, a);
        assert_eq!(k1, k2, "edge_key should not depend on order");
        assert_eq!(k1, e_coord, "edge_key should match stored edge coord for that vertex pair");
    }

    #[test]
    fn generate_from_layout_creates_hexes_vertices_edges() {
        let (resources, numbers) = Board::test_layout();
        let board = Board::generate_from_layout(BoardLayout::Standard, resources, numbers);

        assert_eq!(board.hexes.len(), 19, "radius=2 board should have 19 hexes");
        assert!(!board.vertices.is_empty());
        assert!(!board.edges.is_empty());
    }

    #[test]
    fn test_get_neighbor_vertices() {
        let board = Board::new_standard_board();
        let h = *board.hexes.keys().next().unwrap();
        let neighbors = Board::get_adjacent_hexes(h);
        assert!(!neighbors.is_empty(), "Hex should have neighbors");
        assert!(neighbors.len() == 6, "Hex should have at most 6 adjacent vertices");
    }


    #[test]
    fn test_buildable_vertex_rule() {
        let mut board = Board::new_standard_board();
        let v = pick_buildable_vertex(&board);

        assert!(board.is_buildable_vertex(v));

        board.vertices.get_mut(&v).unwrap().building = Some(VertexBuilding::Settlement);

        // The occupied vertex and each of its real neighbours are now blocked,
        // but a vertex two edges away is still fair game.
        assert!(!board.is_buildable_vertex(v));

        let neighbours = vertex_neighbours(&board, v);
        for n in &neighbours {
            assert!(!board.is_buildable_vertex(*n), "{n:?} is one edge away");
        }

        let two_away = neighbours
            .iter()
            .flat_map(|n| vertex_neighbours(&board, *n))
            .find(|c| *c != v && !neighbours.contains(c))
            .expect("expected a vertex two edges away");

        assert!(
            board.is_buildable_vertex(two_away),
            "{two_away:?} is two edges away and should stay buildable"
        );
    }

    #[test]
    fn is_buildable_vertex_false_for_nonexistent_vertex() {
        let board = Board::new_standard_board();
        assert!(!board.is_buildable_vertex((123456, 654321)));
    }

    #[test]
    fn add_ports_populates_ports_and_has_expected_count() {
        let (resources, numbers) = Board::test_layout();
        let mut board = Board::generate_from_layout(BoardLayout::Standard, resources, numbers);
        assert!(board.ports.is_empty());

        board.add_ports(9);

        assert_eq!(board.ports.len(), 18, "nine harbours, two vertices each");

        // all port entries reference a port with exactly two coords
        for (k, p) in &board.ports {
            assert!(p.coord.contains(k), "port map key should be one of port endpoints");
            match p.port_type {
                PortType::ThreeToOne => {}
                PortType::TwoToOne(_) => {}
            }
        }
    }

    #[test]
    fn is_buildable_vertex_true_on_empty_board_and_false_when_occupied() {
        let mut board = Board::new_standard_board();
        let v = pick_buildable_vertex(&board);

        assert!(board.is_buildable_vertex(v));

        board.vertices.get_mut(&v).unwrap().building = Some(VertexBuilding::Settlement);
        assert!(!board.is_buildable_vertex(v), "occupied vertex must be unbuildable");
    }

    #[test]
    fn is_buildable_vertex_distance_two_rule_blocks_adjacent_vertices() {
        let mut board = Board::new_standard_board();
        let v = pick_buildable_vertex(&board);

        board.vertices.get_mut(&v).unwrap().building = Some(VertexBuilding::Settlement);

        let neighbours = vertex_neighbours(&board, v);
        assert!(!neighbours.is_empty(), "a vertex must have neighbours");

        for n in neighbours {
            assert!(
                !board.is_buildable_vertex(n),
                "neighbor {n:?} should be blocked by distance rule"
            );
        }
    }

    #[test]
    fn build_vertex_sets_building_and_owner() {
        let mut board = Board::new_standard_board();
        let v = pick_any_vertex(&board);

        let pid7 = Uuid::from_u128(7);
        board.build_vertex(pid7, v, VertexBuilding::Settlement);

        let vx = board.vertices.get(&v).unwrap();
        assert_eq!(vx.owner, Some(pid7));
        assert!(vx.building.is_some());
    }

    #[test]
    fn build_edge_sets_building_and_owner() {
        let mut board = Board::new_standard_board();
        let e = pick_any_edge(&board);

        let pid3 = Uuid::from_u128(3);
        board.build_edge(pid3, e, EdgeBuilding::Road);

        let ed = board.edges.get(&e).unwrap();
        assert_eq!(ed.owner, Some(pid3));
        assert!(ed.building.is_some());
    }

    #[test]
    fn is_vertex_connected_to_player_true_when_adjacent_edge_owned_by_player() {
        let mut board = Board::new_standard_board();

        let pid1 = Uuid::from_u128(1);
        let pid2 = Uuid::from_u128(2);

        let e_coord = pick_any_edge(&board);
        let (a, _b) = board.edges.get(&e_coord).unwrap().adjacent_vertices;

        assert!(!board.is_vertex_connected_to_player(a, pid1));

        board.build_edge(pid1, e_coord, EdgeBuilding::Road);
        assert!(board.is_vertex_connected_to_player(a, pid1));
        assert!(!board.is_vertex_connected_to_player(a, pid2));
    }

    #[test]
    fn is_edge_connected_to_player_true_when_adjacent_vertex_owned_by_player() {
        let mut board = Board::new_standard_board();

        let pid5 = Uuid::from_u128(5);
        let pid6 = Uuid::from_u128(6);

        let e_coord = pick_any_edge(&board);
        let (a, _b) = board.edges.get(&e_coord).unwrap().adjacent_vertices;

        assert!(!board.is_edge_connected_to_player(e_coord, pid5));

        board.build_vertex(pid5, a, VertexBuilding::Settlement);

        assert!(board.is_edge_connected_to_player(e_coord, pid5));
        assert!(!board.is_edge_connected_to_player(e_coord, pid6));
    }

    /// Regression test: resources and numbers used to be shuffled as fixed
    /// pairs, so e.g. Ore was the poorest resource in literally every game.
    /// Each layout must produce exactly the hexes, terrain and tokens the
    /// corresponding physical game ships with.
    #[test]
    fn each_layout_has_the_right_hexes_terrain_and_tokens() {
        for (layout, hexes, deserts, tokens) in [
            (BoardLayout::Standard, 19, 1, 18),
            (BoardLayout::Extended, 30, 2, 28),
        ] {
            assert_eq!(
                Board::hex_coords_for(layout).len(),
                hexes,
                "{layout:?} should have {hexes} hexes"
            );

            let board = Board::new_for_layout(layout, 9);
            assert_eq!(board.hexes.len(), hexes);

            let desert_count = board
                .hexes
                .values()
                .filter(|h| h.resource == ResourceType::Desert)
                .count();
            assert_eq!(desert_count, deserts, "{layout:?} desert count");

            let numbered = board.hexes.values().filter(|h| h.number != 0).count();
            assert_eq!(numbered, tokens, "{layout:?} token count");

            assert!(
                board.hexes.values().all(|h| h.number != 7),
                "7 is never a token"
            );
            assert!(
                board.hexes.values().all(|h| (h.resource == ResourceType::Desert) == (h.number == 0)),
                "only deserts are blank"
            );
        }
    }

    /// Every hex must touch another, or the board is in pieces.
    #[test]
    fn the_extended_board_is_one_connected_landmass() {
        let coords: std::collections::HashSet<Coordinates> =
            Board::hex_coords_for(BoardLayout::Extended).into_iter().collect();

        let neighbours = |(q, r): Coordinates| {
            [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)]
                .map(|(dq, dr)| (q + dq, r + dr))
        };

        let start = *coords.iter().next().unwrap();
        let mut seen = std::collections::HashSet::from([start]);
        let mut queue = vec![start];
        while let Some(c) = queue.pop() {
            for n in neighbours(c) {
                if coords.contains(&n) && seen.insert(n) {
                    queue.push(n);
                }
            }
        }

        assert_eq!(seen.len(), coords.len(), "the board must be connected");
    }

    /// Harbours are derived from the coast, so they must land on real coastal
    /// edges on both layouts - never inland, never off the board.
    #[test]
    fn harbours_sit_on_the_coast_of_either_layout() {
        for (layout, port_count) in [(BoardLayout::Standard, 9), (BoardLayout::Extended, 11)] {
            let board = Board::new_for_layout(layout, port_count);

            assert_eq!(
                board.unique_ports().len(),
                port_count,
                "{layout:?} should have {port_count} harbours"
            );

            for port in board.unique_ports() {
                let [a, b] = port.coord;
                assert!(board.vertices.contains_key(&a) && board.vertices.contains_key(&b));

                let edge = Board::edge_key(a, b);
                let touching = board
                    .hexes
                    .keys()
                    .filter(|hex| {
                        let corners = Board::get_adjacent_hexes(**hex);
                        (0..6).any(|i| Board::edge_key(corners[i], corners[(i + 1) % 6]) == edge)
                    })
                    .count();

                assert_eq!(touching, 1, "a harbour must sit on a coastal edge");
            }

            let specific = board
                .unique_ports()
                .iter()
                .filter(|p| matches!(p.port_type, PortType::TwoToOne(_)))
                .count();
            assert_eq!(specific, 5, "one 2:1 harbour per resource");
        }
    }

    #[test]
    fn resource_number_pairing_varies_between_games() {
        use std::collections::{HashMap, HashSet};

        let fingerprint = || {
            let board = Board::new_standard_board();
            let mut by_resource: HashMap<String, Vec<u8>> = HashMap::new();
            for hex in board.hexes.values() {
                by_resource
                    .entry(format!("{:?}", hex.resource))
                    .or_default()
                    .push(hex.number);
            }
            let mut rows: Vec<String> = by_resource
                .into_iter()
                .map(|(resource, mut numbers)| {
                    numbers.sort();
                    format!("{resource}{numbers:?}")
                })
                .collect();
            rows.sort();
            rows.join("|")
        };

        let seen: HashSet<String> = (0..25).map(|_| fingerprint()).collect();

        assert!(
            seen.len() > 1,
            "every board handed the same numbers to the same resources"
        );
    }

    #[test]
    fn every_board_keeps_one_desert_with_no_number() {
        for _ in 0..25 {
            let board = Board::new_standard_board();

            let deserts: Vec<_> = board
                .hexes
                .values()
                .filter(|hex| hex.resource == ResourceType::Desert)
                .collect();

            assert_eq!(deserts.len(), 1, "there must be exactly one desert");
            assert_eq!(deserts[0].number, 0, "the desert must have no number token");

            assert!(
                board
                    .hexes
                    .values()
                    .filter(|hex| hex.resource != ResourceType::Desert)
                    .all(|hex| hex.number != 0 && hex.number != 7),
                "every land hex needs a real token and 7 is never a token"
            );
        }
    }

    #[test]
    fn every_board_uses_the_standard_token_multiset() {
        let mut expected = vec![2, 3, 3, 4, 4, 5, 5, 6, 6, 8, 8, 9, 9, 10, 10, 11, 11, 12];
        expected.sort();

        for _ in 0..10 {
            let board = Board::new_standard_board();
            let mut tokens: Vec<u8> = board
                .hexes
                .values()
                .map(|hex| hex.number)
                .filter(|number| *number != 0)
                .collect();
            tokens.sort();

            assert_eq!(tokens, expected);
        }
    }

    /// Two distinct edges sharing a vertex, plus that vertex.
    fn two_edges_sharing_a_vertex(board: &Board) -> (Coordinates, Coordinates, Coordinates) {
        board
            .edges
            .iter()
            .find_map(|(&e1, edge)| {
                let v = edge.adjacent_vertices.0;
                board.edges.iter().find_map(|(&e2, other)| {
                    let touches =
                        other.adjacent_vertices.0 == v || other.adjacent_vertices.1 == v;
                    if e2 != e1 && touches { Some((e1, v, e2)) } else { None }
                })
            })
            .expect("expected two edges sharing a vertex")
    }

    /// Regression test: a road used to be extendable straight through an
    /// opponent's settlement, which by the rules severs the network.
    #[test]
    fn road_cannot_be_extended_through_an_opponent_building() {
        let mut board = Board::new_standard_board();
        let me = Uuid::from_u128(1);
        let opponent = Uuid::from_u128(2);

        let (e1, shared_vertex, e2) = two_edges_sharing_a_vertex(&board);

        board.build_edge(me, e1, EdgeBuilding::Road);
        assert!(
            board.is_edge_connected_to_player(e2, me),
            "an open junction should connect my two edges"
        );

        board.build_vertex(opponent, shared_vertex, VertexBuilding::Settlement);

        assert!(
            !board.is_edge_connected_to_player(e2, me),
            "an opponent's building must cut the road network at that junction"
        );
    }

    /// My own building must not block me.
    #[test]
    fn road_can_be_extended_through_my_own_building() {
        let mut board = Board::new_standard_board();
        let me = Uuid::from_u128(1);

        let (e1, shared_vertex, e2) = two_edges_sharing_a_vertex(&board);

        board.build_edge(me, e1, EdgeBuilding::Road);
        board.build_vertex(me, shared_vertex, VertexBuilding::Settlement);

        assert!(board.is_edge_connected_to_player(e2, me));
    }

    #[test]
    fn is_edge_connected_to_player_true_when_adjacent_edge_owned_by_player_via_shared_vertex() {
        let mut board = Board::new_standard_board();

        let pid9 = Uuid::from_u128(9);
        let pid10 = Uuid::from_u128(10);

        // Pick an existing edge e1 and one of its endpoint vertices v.
        let (&e1, edge1) = board
            .edges
            .iter()
            .next()
            .expect("board must have at least one edge");
        let (v, _other) = edge1.adjacent_vertices;
    
        // Find another existing edge e2 that also touches v.
        let e2 = board
            .edges
            .iter()
            .find_map(|(&ek, e)| {
                if ek != e1 && (e.adjacent_vertices.0 == v || e.adjacent_vertices.1 == v) {
                    Some(ek)
                } else {
                    None
                }
            })
            .expect("expected at least two edges sharing a vertex");
    
        board.build_edge(pid9, e1, EdgeBuilding::Road);

        assert!(board.is_edge_connected_to_player(e2, pid9));
        assert!(!board.is_edge_connected_to_player(e2, pid10));
    }
}

