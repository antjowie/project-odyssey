/// Logic for creating graphs from rails. We have multiple graph types that we
/// generate such as:
/// * a graph for pathfinding, we need nodes per intersection
/// * a graph for traffic control, so we can store data on edges to see if they
/// are occupied.
///
/// We store this as seperate graphs, as construction can be done on worked
/// threads and we want to optimize the graphs for algorithm
use bevy::prelude::*;
use petgraph::prelude::*;

pub fn rail_graph_plugin(app: &mut App) {
    {
        app.add_systems(Startup, test_rail_graph);
        app.add_systems(Update, debug_rail_graph);
    }
}

#[derive(Component, Default)]
struct RailGraph(StableDiGraph<RailVertex, RailEdge>);

struct RailVertex {
    pos: Vec3,
}

#[derive(Default)]
struct RailEdge {}

fn test_rail_graph(mut c: Commands) {
    let mut graph = StableDiGraph::<RailVertex, RailEdge>::default();
    let u = graph.add_node(RailVertex {
        pos: Vec3::new(0.0, 0.01, 0.0),
    });
    let v = graph.add_node(RailVertex {
        pos: Vec3::new(10.0, 0.01, 0.0),
    });
    let w = graph.add_node(RailVertex {
        pos: Vec3::new(10.0, 0.01, 10.0),
    });
    let x = graph.add_node(RailVertex {
        pos: Vec3::new(0.0, 0.01, 10.0),
    });

    graph.add_edge(u, v, RailEdge::default());
    graph.add_edge(v, w, RailEdge::default());
    graph.add_edge(w, x, RailEdge::default());

    c.spawn(RailGraph(graph));
}

fn debug_rail_graph(mut gizmos: Gizmos, q: Query<&RailGraph>) {
    q.into_iter().for_each(|graph| {
        graph.0.edge_indices().for_each(|edge| {
            if let Some((start, end)) = graph.0.edge_endpoints(edge) {
                let start = graph.0.node_weight(start).unwrap();
                let end = graph.0.node_weight(end).unwrap();
                gizmos.arrow(start.pos, end.pos, Color::WHITE);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use petgraph::algo::dijkstra;

    use super::*;

    #[test]
    fn test_add_edge() {
        let mut graph = RailGraph::default();
    }
}
