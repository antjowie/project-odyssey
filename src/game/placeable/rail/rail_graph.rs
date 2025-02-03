//! Logic for creating graphs from rails. We have multiple graph types that we
//! generate such as:
//! * a graph for pathfinding, we need nodes per intersection
//! * a graph for traffic control, so we can store data on edges to see if they
//! are occupied.
//!
//! We store this as seperate graphs, as construction can be done on worked
//! threads and we want to optimize the graphs for algorithm
use bevy::{color::palettes::tailwind::GRAY_500, prelude::*};
use petgraph::{algo::astar, prelude::*};
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

pub type RailGraphNodeId = NodeIndex<u32>;
pub type RailGraphEdgeId = EdgeIndex<u32>;

#[derive(Resource, Default)]
pub struct RailGraph(pub StableDiGraph<RailNode, RailEdge>);
impl RailGraph {
    pub fn get_path(
        &self,
        from: NodeIndex,
        to: NodeIndex,
        intersections: Res<RailIntersections>,
    ) -> Vec<u32> {
        // astar()
        // dijkstra(graph, start, goal, edge_cost)
        // let mut predecessor = vec![NodeIndex::end(); gr.node_count()];
        // depth_first_search(&gr, Some(start), |event| {
        //     if let DfsEvent::TreeEdge(u, v) = event {
        //         predecessor[v.index()] = u;
        //         if v == goal {
        //             return Control::Break(v);
        //         }
        //     }
        //     Control::Continue
        // });
        // let mut next = goal;
        // let mut path = vec![next];
        // while next != start {
        //     let pred = predecessor[next.index()];
        //     path.push(pred);
        //     next = pred;
        // }
        // path.reverse();
        // assert_eq!(&path, &[n(0), n(2), n(4), n(5)]);

        todo!()
    }
}

pub struct RailNode {
    pub intersection_id: Uuid,
}

#[derive(Default, Clone, Copy)]
pub struct RailEdge {
    pub length: f32,
}

fn on_rail_intersection_changed(
    mut ev: EventReader<RailIntersectionChangedEvent>,
    mut graph: ResMut<RailGraph>,
    mut rails: Query<(&mut Rail, &Spline)>,
    intersections: Res<RailIntersections>,
) {
    for ev in ev.read() {
        // Update edges from and to
        let intersection = intersections.intersections.get(&ev.0).unwrap();
        let nav_id = intersection.nav_id;
        intersection.curves().iter().for_each(|x| {
            let (mut rail, spline) = rails.get_mut(*x).unwrap();
            let end = rail.far_intersection(&intersection.collision.center.into(), &intersections);
            let end_id = end.nav_id;
            let edge = RailEdge {
                length: spline.curve_length(),
            };

            let to_end = graph.0.update_edge(nav_id, end_id, edge);
            let to_start = graph.0.update_edge(end_id, nav_id, edge);

            if rail.nav_to_end.is_none() {
                if rail.joints[0].intersection_id == intersection.uuid {
                    rail.nav_to_end = Some(to_end);
                    rail.nav_to_start = Some(to_start);
                } else {
                    rail.nav_to_end = Some(to_start);
                    rail.nav_to_start = Some(to_end);
                }
            }
        });
    }
}

fn on_rail_intersection_removed(
    mut ev: EventReader<RailIntersectionRemovedEvent>,
    mut graph: ResMut<RailGraph>,
) {
    for ev in ev.read() {
        graph.0.remove_node(ev.0.nav_id);
    }
}

fn on_rail_removed(mut ev: EventReader<RailRemovedEvent>, mut graph: ResMut<RailGraph>) {
    for ev in ev.read() {
        let rail = ev.0;
        if let Some(to_end) = rail.nav_to_end {
            graph.0.remove_edge(to_end);
        }
        if let Some(to_start) = rail.nav_to_start {
            graph.0.remove_edge(to_start);
        }
    }
}

fn debug_rail_graph(
    mut gizmos: Gizmos,
    graph: Res<RailGraph>,
    intersections: Res<RailIntersections>,
) {
    graph.0.edge_indices().for_each(|edge| {
        if let Some((start, end)) = graph.0.edge_endpoints(edge) {
            let start = graph.0.node_weight(start).unwrap();
            let end = graph.0.node_weight(end).unwrap();
            let start = intersections
                .intersections
                .get(&start.intersection_id)
                .unwrap()
                .collision
                .center;
            let end = intersections
                .intersections
                .get(&end.intersection_id)
                .unwrap()
                .collision
                .center;
            let to_end = (end - start).normalize();

            gizmos
                .arrow(start.into(), (end - to_end).into(), GRAY_500)
                .with_tip_length(2.0);
        }
    });

    graph.0.node_weights().for_each(|node| {
        let i = intersections
            .intersections
            .get(&node.intersection_id)
            .unwrap();
        gizmos.sphere(i.collision.center, 1.0, GRAY_500);
    });
}
