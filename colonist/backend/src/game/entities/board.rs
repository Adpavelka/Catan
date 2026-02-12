use crate::game::entities::building::{EdgeBuilding, VertexBuilding};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use shared::{BoardInfo, BuildingInfo, HexInfo, PortInfo, ResourceType};
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

    fn generate_from_layout(resources: Vec<ResourceType>, numbers: Vec<u8>) -> Self {
        let mut board = Board::default();

        let radius = 2;
        let mut hex_coords = Vec::new();
        for q in -radius..=radius {
            for r in -radius..=radius {
                if ((q + r) as i32).abs() <= radius {
                    hex_coords.push((q, r));
                }
            }
        }

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


    fn test_layout() -> (Vec<ResourceType>, Vec<u8>) { // standard counts: wood 4, sheep 4, wheat 4, brick 3, ore 3, desert 1 => 19 hexes
        use ResourceType::*;
        let mut resources = Vec::with_capacity(19);
        resources.extend(repeat_n(Wood, 4));
        resources.extend(repeat_n(Sheep, 4));
        resources.extend(repeat_n(Wheat, 4));
        resources.extend(repeat_n(Brick, 3));
        resources.extend(repeat_n(Ore, 3));
        resources.push(Desert);

        let numbers = vec![5, 2, 6, 3, 8, 10, 9, 12, 11, 4, 8, 10, 9, 4, 5, 6, 3, 11, 0]; // fixed order
        (resources, numbers)
    }

    fn random_layout() -> (Vec<ResourceType>, Vec<u8>) {
        let (mut resources, mut numbers) = Self::test_layout();

        //#[cfg(not(test))]
        {
            use rand::seq::SliceRandom;
            use rand::thread_rng;

            let mut rng = thread_rng();

            let mut combined: Vec<(ResourceType, u8)> = resources.into_iter().zip(numbers.into_iter()).collect();
            combined.shuffle(&mut rng);

            let (res, nums): (Vec<_>, Vec<_>) = combined.into_iter().unzip();
            resources = res;
            numbers = nums;
        }

        (resources, numbers)
    }


    fn add_ports(&mut self) {
        use PortType::*;

        let port_coords: [[Coordinates; 2]; 9] = [
            [(2, -7), (4, -8)],
            [(7, -8), (8, -7)],
            [(8, -4), (7, -2)],
            [(5, 2), (4, 4)],
            [(1, 7), (-1, 8)],
            [(-4, 8), (-5, 7)],
            [(-7, 5), (-8, 4)],
            [(-7, -1), (-8, 1)],
            [(-4, -4), (-2, -5)],
        ];

        let mut port_types = vec![
            TwoToOne(ResourceType::Brick),
            TwoToOne(ResourceType::Wood),
            TwoToOne(ResourceType::Sheep),
            TwoToOne(ResourceType::Wheat),
            TwoToOne(ResourceType::Ore),
            ThreeToOne,
            ThreeToOne,
            ThreeToOne,
            ThreeToOne,
        ];

        let mut rng = rand::thread_rng();
        port_types.shuffle(&mut rng);

        let mut ports_map = HashMap::new();
        for (coords, port_type) in port_coords.iter().zip(port_types.into_iter()) {
            let port = Port {
                coord: *coords,
                port_type,
            };

            ports_map.insert(coords[0], port.clone());
            ports_map.insert(coords[1], port);
        }

        self.ports = ports_map;
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
        let (resources, numbers) = Self::random_layout();
        let mut board = Self::generate_from_layout(resources, numbers);
        board.add_ports();
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
            if let Some(v) = self.vertices.get(&vcoord) {
                if let Some(id) = v.owner {
                    if id == player_id { 
                        return true;
                    }
                }

                if v.adjacent_edges.iter().any(|&ek| {
                    if let Some(e) = self.edges.get(&ek) {
                        if let Some(id) = e.owner {
                            return id == player_id;
                        }
                    }
                    false
                }) {
                    return true;
                }
            }

            false
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

        let desert_count = resources.iter().filter(|&&r| r == ResourceType::Desert).count();
        assert_eq!(desert_count, 1);
        assert_eq!(*numbers.last().unwrap(), 0);
    }

    #[test]
    fn random_layout_returns_19_items_and_includes_desert_and_zero_end() {
        let (resources, numbers) = Board::random_layout();
        assert_eq!(resources.len(), 19);
        assert_eq!(numbers.len(), 19);

        let desert_count = resources.iter().filter(|&&r| r == ResourceType::Desert).count();
        assert_eq!(desert_count, 1);
        assert_eq!(*numbers.last().unwrap(), 0);
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
        let board = Board::generate_from_layout(resources, numbers);

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
        let v = *board.vertices.keys().next().unwrap();

        assert!(board.is_buildable_vertex(v));

        board.vertices.get_mut(&v).unwrap().building = Some(VertexBuilding::Settlement);

        let v1 = (v.0, v.1 + 1);
        let v2 = (v.0 + 1, v.1);

        assert!(!board.is_buildable_vertex(v1));
        assert!(!board.is_buildable_vertex(v2));
    }

    #[test]
    fn is_buildable_vertex_false_for_nonexistent_vertex() {
        let board = Board::new_standard_board();
        assert!(!board.is_buildable_vertex((123456, 654321)));
    }

    #[test]
    fn add_ports_populates_ports_and_has_expected_count() {
        let (resources, numbers) = Board::test_layout();
        let mut board = Board::generate_from_layout(resources, numbers);
        assert!(board.ports.is_empty());

        board.add_ports();

        assert_eq!(board.ports.len(), 18);

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

        for n in Board::get_adjacent_hexes(v) {
            if board.vertices.contains_key(&n) {
                assert!(
                    !board.is_buildable_vertex(n),
                    "neighbor {n:?} should be blocked by distance rule"
                );
            }
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

