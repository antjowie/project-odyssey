//! Logic for creating graphs from rails. We have multiple graph types that we
//! generate such as:
//! * a graph for pathfinding, we need nodes per intersection
//! * a graph for traffic control, so we can store data on edges to see if they
//! are occupied.
//!
//! We store this as seperate graphs, as construction can be done on worked
//! threads and we want to optimize the graphs for algorithm
use avian3d::math::FRAC_PI_2;
use bevy::{color::palettes::tailwind::GRAY_500, prelude::*};
use petgraph::{algo::astar, prelude::*};
use uuid::Uuid;

use super::*;

pub fn rail_graph_plugin(app: &mut App) {
    {
        app.insert_resource(RailGraph::default());
        app.add_systems(
            Update,
            debug_rail_graph.run_if(in_player_state(PlayerState::Building)),
        );
        app.add_systems(
            // PostUpdate ensures that components for new rails are added
            PostUpdate,
            (
                on_rail_intersection_changed.run_if(on_event::<RailIntersectionChangedEvent>),
                on_rail_intersection_removed.run_if(on_event::<RailIntersectionRemovedEvent>),
                on_rail_removed.run_if(on_event::<RailRemovedEvent>),
            ),
        );
    }
}

type RailGraphNodeId = NodeIndex<u32>;
type RailGraphEdgeId = EdgeIndex<u32>;

struct RailGraphNode {
    intersection_id: Uuid,
    pos: Vec3,
    forward: Dir3,
}

#[derive(Clone, Copy)]
struct RailGraphNodeBinding {
    /// Right aligns with the intersection right
    right: RailGraphNodeId,
    left: RailGraphNodeId,
}

struct RailGraphEdge {
    entity: Entity,
    length: f32,
    /// Direction to move along with to follow this edge
    forward: Dir3,
}

#[derive(Clone, Copy, Default)]
struct RailGraphEdgeBinding {
    to_end: Option<RailGraphEdgeId>,
    to_start: Option<RailGraphEdgeId>,
}

#[derive(Resource, Default)]
pub struct RailGraph {
    graph: StableDiGraph<RailGraphNode, RailGraphEdge>,
    intersection_to_binding: HashMap<Uuid, RailGraphNodeBinding>,
    rail_to_edge: HashMap<Entity, RailGraphEdgeBinding>,
}

pub struct RailGraphTraverseResult {
    pub cost: u32,
    pub traversal: Vec<RailGraphTraversal>,
    pub end_position: Vec3,
}

impl RailGraphTraverseResult {
    pub fn new(
        cost: Option<u32>,
        traversal: Vec<RailGraphTraversal>,
        start_pos: Vec3,
        end_pos: Vec3,
        start_rail: Entity,
        rails: &Query<(&Rail, &Spline)>,
    ) -> Self {
        let mut result = RailGraphTraverseResult {
            cost: 0,
            traversal,
            end_position: end_pos,
        };

        let points = result.points(&start_pos, &end_pos, start_rail, rails);
        result.cost = cost.unwrap_or(
            points
                .iter()
                .zip(points.iter().skip(1))
                .fold(0.0, |acc, (x, y)| acc + x.distance(*y))
                .round() as u32,
        );
        result
    }

    pub fn validate(&self, rails: &Query<(), With<Rail>>) -> bool {
        self.traversal
            .iter()
            .any(|x| rails.contains(x.rail) == false)
            == false
    }

    pub fn points(
        &self,
        start_pos: &Vec3,
        end_pos: &Vec3,
        current_rail: Entity,
        rails: &Query<(&Rail, &Spline)>,
    ) -> Vec<Vec3> {
        let mut points = vec![*start_pos];
        let len = self.traversal.len();
        let mut found_first_rail = false;
        points.append(
            &mut self
                .traversal
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    let (_rail, spline) = rails.get(x.rail).unwrap();
                    let mut points = spline.curve_points_projected().to_owned();

                    if !x.rail_at_start {
                        points.reverse();
                    }

                    if x.rail == current_rail && found_first_rail == false {
                        found_first_rail = true;
                        let t = spline.t_from_pos(&start_pos);
                        if x.rail_at_start {
                            points = points
                                .into_iter()
                                .filter(|x| spline.t_from_pos(x) > t)
                                .collect();
                        } else {
                            points = points
                                .into_iter()
                                .filter(|x| spline.t_from_pos(x) < t)
                                .collect();
                        }
                    }

                    if found_first_rail == false {
                        points.clear();
                        return points;
                    }

                    if i == len - 1 {
                        let t = spline.t_from_pos(&end_pos);
                        if x.rail_at_start {
                            points = points
                                .into_iter()
                                .filter(|x| spline.t_from_pos(x) < t)
                                .collect();
                        } else {
                            points = points
                                .into_iter()
                                .filter(|x| spline.t_from_pos(x) > t)
                                .collect();
                        }
                    }
                    points
                })
                .flatten()
                .collect(),
        );
        points.push(*end_pos);
        points.dedup();
        points
    }
}

/// It's safe to store copies RailIntersections, any modifications should kick of a new path search
pub struct RailGraphTraversal {
    pub from: RailIntersection,
    pub to: RailIntersection,
    pub rail: Entity,
    // If from and start of rail are at same pos
    // If this is not the case, we are traversing in a negative direction over the rail
    pub rail_at_start: bool,
}

impl RailGraph {
    fn add_intersection(&mut self, intersection: &RailIntersection) -> RailGraphNodeBinding {
        let pos: Vec3 = intersection.collision.center.into();
        let binding = RailGraphNodeBinding {
            right: self.graph.add_node(RailGraphNode {
                intersection_id: intersection.uuid,
                pos: pos - Quat::from_rotation_y(FRAC_PI_2) * intersection.right_forward.as_vec3(),
                forward: intersection.right_forward,
            }),
            left: self.graph.add_node(RailGraphNode {
                intersection_id: intersection.uuid,
                pos: pos + Quat::from_rotation_y(FRAC_PI_2) * intersection.right_forward.as_vec3(),
                forward: -intersection.right_forward,
            }),
        };

        self.intersection_to_binding
            .insert(intersection.uuid, binding);
        binding
    }

    fn get_or_add_intersection(&mut self, intersection: &RailIntersection) -> RailGraphNodeBinding {
        if self
            .intersection_to_binding
            .contains_key(&intersection.uuid)
        {
            *self
                .intersection_to_binding
                .get(&intersection.uuid)
                .unwrap()
        } else {
            self.add_intersection(intersection)
        }
    }

    fn get_or_add_edge(&mut self, rail: &Entity) -> RailGraphEdgeBinding {
        if self.rail_to_edge.contains_key(rail) {
            *self.rail_to_edge.get(rail).unwrap()
        } else {
            RailGraphEdgeBinding::default()
        }
    }

    /// Returns a vec of intersections to travel through
    /// The contained items will include the start and end rail
    pub fn get_path(
        &self,
        from_t: f32,
        from_rail: Entity,
        current_dir: &Dir3,
        next_intersection: &RailIntersection,
        next_intersection_approach_dir: &Dir3,
        to_rail: Entity,
        to_pos: &Vec3,
        intersections: &RailIntersections,
        rails: &Query<(&Rail, &Spline)>,
    ) -> Option<RailGraphTraverseResult> {
        let (start_rail, start_spline) = rails
            .get(from_rail)
            .expect("Traversing from non-existing rail");
        let (_end_rail, end_spline) = rails.get(to_rail).expect("Traversing to non-existing rail");

        // Handle case where we nav to same rail and we don't have to path find
        let travel_right = start_spline.forward(from_t).dot(current_dir.as_vec3()) > 0.0;
        let from_pos = start_spline.projected_position(from_t);
        let to_pos = end_spline.projected_position(end_spline.t_from_pos(to_pos));

        if from_rail == to_rail {
            let to_t = start_spline.t_from_pos(&to_pos);
            if (travel_right && to_t > from_t) || (travel_right == false && to_t < from_t) {
                let traversal = RailGraphTraversal {
                    from: start_rail
                        .far_intersection(
                            &next_intersection.collision.center.into(),
                            &intersections,
                        )
                        .to_owned(),
                    to: next_intersection.to_owned(),
                    rail: from_rail,
                    rail_at_start: travel_right,
                };

                return Some(RailGraphTraverseResult::new(
                    None,
                    vec![traversal],
                    from_pos,
                    to_pos,
                    from_rail,
                    rails,
                ));
            }
        }

        let rail_binding = self.rail_to_edge.get(&to_rail);
        if rail_binding.is_none() {
            return None;
        }
        let rail_binding = rail_binding.unwrap();

        let start = self.intersection_to_binding.get(&next_intersection.uuid);
        if start.is_none() {
            return None;
        }
        let start = start.unwrap();

        let start = if next_intersection
            .right_forward
            .dot(next_intersection_approach_dir.as_vec3())
            > 0.0
        {
            start.right
        } else {
            start.left
        };

        let mut end_intersection_id = next_intersection.uuid;
        let reached_goal = |node| {
            self.graph
                .edges_directed(node, Direction::Outgoing)
                .find(|x| {
                    if rail_binding.to_end.is_some_and(|y| x.id() == y) {
                        let node = self
                            .graph
                            .edge_endpoints(rail_binding.to_end.unwrap())
                            .unwrap()
                            .1;
                        end_intersection_id = self.graph.node_weight(node).unwrap().intersection_id;
                        true
                    } else if rail_binding.to_start.is_some_and(|y| x.id() == y) {
                        let node = self
                            .graph
                            .edge_endpoints(rail_binding.to_start.unwrap())
                            .unwrap()
                            .1;
                        end_intersection_id = self.graph.node_weight(node).unwrap().intersection_id;
                        true
                    } else {
                        false
                    }
                })
                .is_some()
        };

        let path = astar(
            &self.graph,
            start,
            reached_goal,
            |edge| {
                let edge = edge.weight();
                edge.length.round() as u32
            },
            |node| {
                let node = self.graph.node_weight(node).unwrap();
                node.pos.distance_squared(to_pos).round() as u32
            },
        );

        if let Some(path) = path {
            let mut traversal = vec![];
            traversal.push(
                intersections
                    .intersections
                    .get(&start_rail.joints[if travel_right { 0 } else { 1 }].intersection_id)
                    .unwrap(),
            );
            traversal.append(
                &mut path
                    .1
                    .into_iter()
                    .map(|x| {
                        intersections
                            .intersections
                            .get(&self.graph.node_weight(x).unwrap().intersection_id)
                            .unwrap()
                    })
                    .collect(),
            );
            traversal.push(
                intersections
                    .intersections
                    .get(&end_intersection_id)
                    .unwrap(),
            );

            Some(RailGraphTraverseResult::new(
                Some(path.0),
                traversal
                    .iter()
                    .zip(traversal.iter().skip(1))
                    .map(|(x, y)| {
                        let mut rail_at_start = false;
                        let rail = *x
                            .curves()
                            .iter()
                            .find(|x| {
                                let (rail, _) = rails.get(**x).unwrap();
                                if rail.joints[0].intersection_id == y.uuid {
                                    rail_at_start = false;
                                    true
                                } else if rail.joints[1].intersection_id == y.uuid {
                                    rail_at_start = true;
                                    true
                                } else {
                                    false
                                }
                            })
                            .unwrap();
                        RailGraphTraversal {
                            from: *x.to_owned(),
                            to: *y.to_owned(),
                            rail,
                            rail_at_start,
                        }
                    })
                    .collect(),
                from_pos,
                to_pos,
                from_rail,
                &rails,
            ))
        } else {
            None
        }
    }
}

fn on_rail_intersection_changed(
    mut ev: EventReader<RailIntersectionChangedEvent>,
    mut graph: ResMut<RailGraph>,
    rails: Query<(&Rail, &Spline)>,
    intersections: Res<RailIntersections>,
) {
    for ev in ev.read() {
        let intersection = intersections
            .intersections
            .get(&ev.0)
            .expect("Could not find intersection");

        let binding = graph.get_or_add_intersection(intersection);

        let mut func = |forward, binding_source| {
            intersection.curve_options(&forward).iter().for_each(|x| {
                let (rail, spline) = rails.get(*x).unwrap();
                let end =
                    rail.far_intersection(&intersection.collision.center.into(), &intersections);

                let to_end;
                let approach_dir = if end.uuid == rail.joints[0].intersection_id {
                    to_end = false;
                    -spline.controls()[0].forward
                } else {
                    to_end = true;
                    -spline.controls()[1].forward
                };

                let edge = RailGraphEdge {
                    entity: *x,
                    length: spline.curve_length(),
                    forward: approach_dir,
                };

                // All of these rails we wanna match in the ongoing direction
                let end_binding = graph.get_or_add_intersection(end);
                let end_binding = if approach_dir.dot(end.right_forward.as_vec3()) > 0.0 {
                    end_binding.right
                } else {
                    end_binding.left
                };

                let id = graph.graph.update_edge(binding_source, end_binding, edge);
                let mut binding = graph.get_or_add_edge(x);
                if to_end {
                    binding.to_end = Some(id)
                } else {
                    binding.to_start = Some(id)
                };
                graph.rail_to_edge.insert(*x, binding);
            });
        };

        func(intersection.right_forward, binding.right);
        func(-intersection.right_forward, binding.left);
    }
}

fn on_rail_intersection_removed(
    mut ev: EventReader<RailIntersectionRemovedEvent>,
    mut graph: ResMut<RailGraph>,
) {
    for ev in ev.read() {
        let node = graph.intersection_to_binding.remove(&ev.0.uuid).unwrap();
        graph.graph.remove_node(node.left);
        graph.graph.remove_node(node.right);
    }
}

fn on_rail_removed(mut ev: EventReader<RailRemovedEvent>, mut graph: ResMut<RailGraph>) {
    for ev in ev.read() {
        let binding = graph.rail_to_edge.remove(&ev.0).unwrap();
        if let Some(id) = binding.to_end {
            graph.graph.remove_edge(id);
        }
        if let Some(id) = binding.to_start {
            graph.graph.remove_edge(id);
        }
    }
}

fn debug_rail_graph(mut gizmos: Gizmos, graph: Res<RailGraph>) {
    graph.graph.edge_indices().for_each(|edge| {
        if let Some((start, end)) = graph.graph.edge_endpoints(edge) {
            let start = graph.graph.node_weight(start).unwrap().pos;
            let end = graph.graph.node_weight(end).unwrap().pos;
            let to_end = (end - start).normalize();

            gizmos
                .arrow(start.into(), (end - to_end).into(), GRAY_500)
                .with_tip_length(2.0);
        }
    });

    graph.graph.node_weights().for_each(|node| {
        gizmos.sphere(node.pos, 1.0, GRAY_500);
        gizmos.arrow(
            node.pos,
            node.pos + node.forward.as_vec3(),
            Color::srgb(1.0, 0.0, 0.0),
        );
    });
}
