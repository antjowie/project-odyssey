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
use petgraph::{algo::astar, prelude::*, visit::NodeRef};
use uuid::Uuid;

use super::*;

pub fn rail_graph_plugin(app: &mut App) {
    {
        app.insert_resource(RailGraph::default());
        app.add_systems(Update, debug_rail_graph);
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
    pub fn get_path(
        &self,
        from_intersection: &RailIntersection,
        dir: &Dir3,
        to_rail: Entity,
        to_pos: &Vec3,
    ) -> Option<Vec<Uuid>> {
        let rail_binding = self.rail_to_edge.get(&to_rail);
        if rail_binding.is_none() {
            return None;
        }
        let rail_binding = rail_binding.unwrap();

        let start = self.intersection_to_binding.get(&from_intersection.uuid);
        if start.is_none() {
            return None;
        }
        let start = start.unwrap();

        let start = if from_intersection.right_forward.dot(dir.as_vec3()) > 0.0 {
            start.right
        } else {
            start.left
        };

        let reached_goal = |node| {
            self.graph
                .edges_directed(node, Direction::Outgoing)
                .find(|x| {
                    rail_binding.to_end.is_some_and(|y| x.id() == y)
                        || rail_binding.to_start.is_some_and(|y| x.id() == y)
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
                node.pos.distance_squared(*to_pos).round() as u32
            },
        );

        path.map(|x| {
            x.1.iter()
                .map(|x| self.graph.node_weight(*x).unwrap().intersection_id)
                .collect()
        })
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
