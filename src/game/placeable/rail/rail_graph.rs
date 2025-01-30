/// Logic for creating graphs from rails. We have multiple graph types that we
/// generate such as:
/// * a graph for pathfinding, we need nodes per intersection
/// * a graph for traffic control, so we can store data on edges to see if they
/// are occupied.
///
/// We store this as seperate graphs, as construction can be done on worked
/// threads and we want to optimize the graphs for algorithm
use bevy::{color::palettes::tailwind::GRAY_500, prelude::*};
pub use petgraph::graph::node_index as NodeIndex;
use petgraph::{prelude::*, visit::Visitable};
use uuid::Uuid;

use super::RailIntersections;

pub fn rail_graph_plugin(app: &mut App) {
    {
        app.add_event::<RailIntersectionAddedEvent>();
        app.add_event::<RailIntersectionRemovedEvent>();
        app.add_event::<RailAddedEvent>();
        app.add_event::<RailRemovedEvent>();
        app.insert_resource(RailGraph::default());
        app.add_systems(
            Update,
            (
                debug_rail_graph,
                generate_rail_graph.run_if(resource_changed::<RailIntersections>),
            ),
        );
    }
}

#[derive(Event)]
pub struct RailIntersectionAddedEvent;
#[derive(Event)]
pub struct RailIntersectionRemovedEvent;
#[derive(Event)]
pub struct RailAddedEvent;
#[derive(Event)]
pub struct RailRemovedEvent;

#[derive(Resource, Default)]
pub struct RailGraph(StableDiGraph<RailNode, RailEdge>);
impl RailGraph {
    pub fn get_path(
        &self,
        from: NodeIndex,
        to: NodeIndex,
        intersections: Res<RailIntersections>,
    ) -> Vec<u32> {
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

#[derive(Default)]
pub struct RailEdge {
    pub length: f32,
}

fn generate_rail_graph() {}

fn debug_rail_graph(
    mut gizmos: Gizmos,
    graph: Res<RailGraph>,
    intersections: Res<RailIntersections>,
) {
    graph.0.edge_indices().for_each(|edge| {
        if let Some((start, end)) = graph.0.edge_endpoints(edge) {
            let start = graph.0.node_weight(start).unwrap();
            let end = graph.0.node_weight(end).unwrap();
            let start_i = intersections
                .intersections
                .get(&start.intersection_id)
                .unwrap();
            let end_i = intersections
                .intersections
                .get(&end.intersection_id)
                .unwrap();
            gizmos.arrow(
                start_i.collision.center.into(),
                end_i.collision.center.into(),
                GRAY_500,
            );
        }
    });
}
