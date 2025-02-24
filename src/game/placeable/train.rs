use super::{rail::rail_graph::*, *};
use rail::Rail;
use uuid::Uuid;

pub(super) fn train_plugin(app: &mut App) {
    app.add_systems(Startup, load_train_asset);
    app.add_systems(
        Update,
        (
            handle_train_placement.run_if(
                in_player_state(PlayerState::Building).and(is_placeable_preview(Placeable::Train)),
            ),
            move_trains_with_plan,
            calculate_plan,
        )
            .in_set(GameSet::Update),
    );
}

#[derive(Component)]
#[require(Placeable(||Placeable::Train), Name(|| Name::new("Train")), Selectable)]
pub struct Train {
    /// The alpha on the current rail the train is traversing
    pub t: f32,
    pub rail: Entity,
    pub plan: Option<RailGraphTraverseResult>,
}

impl Train {
    pub fn traverse(
        &self,
        distance: f32,
        t: f32,
        forward: Dir3,
        rail_id: Entity,
        rail: &Rail,
        spline: &Spline,
        rails: &Query<(&Rail, &Spline)>,
        intersections: &RailIntersections,
        plan: &RailGraphTraverseResult,
    ) -> TrainTraverseResult {
        match rail.traverse(t, &forward, distance, spline) {
            TraverseResult::Intersection {
                t,
                pos,
                forward,
                remaining_distance,
                intersection_id,
            } => {
                let intersection = intersections.intersections.get(&intersection_id).unwrap();
                let options = intersection.curve_options(&forward);
                if options.is_empty() {
                    // We'll remain here, stuck forever...
                    TrainTraverseResult {
                        t,
                        pos,
                        forward,
                        rail: rail_id,
                        status: TrainTraverseStatus::ReachedDestination,
                    }
                } else {
                    let decision = plan
                        .traversal
                        .iter()
                        .find(|x| x.from.uuid == intersection_id)
                        .expect("The passed plan is invalid, could not find next intersection");

                    let new_rail_id = decision.rail;
                    let (new_rail, new_spline) = rails.get(new_rail_id).unwrap();
                    let t = new_spline.t_from_pos(&pos).round();
                    self.traverse(
                        remaining_distance,
                        t,
                        forward,
                        new_rail_id,
                        new_rail,
                        new_spline,
                        rails,
                        intersections,
                        &plan,
                    )
                }
            }
            TraverseResult::End { t, pos, forward } => TrainTraverseResult {
                t,
                pos,
                forward,
                rail: rail_id,
                status: if pos.distance_squared(plan.end_position) < 0.1 {
                    TrainTraverseStatus::ReachedDestination
                } else {
                    TrainTraverseStatus::InTransit
                },
            },
        }
    }

    pub fn next_intersection(
        &self,
        transform: &Transform,
        rail: &Rail,
        spline: &Spline,
    ) -> (Uuid, Dir3) {
        match rail.traverse(
            self.t,
            &transform.forward(),
            spline.curve_length() * 2.0,
            &spline,
        ) {
            TraverseResult::Intersection {
                t: _,
                pos: _,
                forward,
                remaining_distance: _,
                intersection_id,
            } => (intersection_id, forward),
            _ => {
                panic!("How can we not hit an intersection if we traverse the entire rail?");
            }
        }
    }
}

pub struct TrainTraverseResult {
    pub t: f32,
    pub pos: Vec3,
    pub forward: Dir3,
    pub rail: Entity,
    pub status: TrainTraverseStatus,
}

#[derive(PartialEq, Eq)]
pub enum TrainTraverseStatus {
    InTransit,
    ReachedDestination,
    Aborted,
}

#[derive(Resource)]
pub struct TrainAsset {
    pub scene: Handle<Scene>,
    pub scale: Vec3,
}

fn load_train_asset(mut c: Commands, asset_server: Res<AssetServer>) {
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/train.glb"));

    c.insert_resource(TrainAsset {
        scene,
        scale: Vec3::splat(RAIL_SEGMENT_WIDTH),
    });
}

fn move_trains_with_plan(
    mut q: Query<(&mut Transform, &mut Train)>,
    rails: Query<(&Rail, &Spline)>,
    rails_filtered: Query<(), With<Rail>>,
    time: Res<Time>,
    intersections: Res<RailIntersections>,
    mut gizmos: Gizmos,
    graph: Res<RailGraph>,
) {
    q.iter_mut().for_each(|(mut t, mut train)| {
        let (rail, spline) = rails.get(train.rail).unwrap();
        // Validate plan
        if let Some(plan) = &train.plan {
            if plan.validate(&rails_filtered) == false {
                let next = train.next_intersection(&t, rail, spline);
                let to_rail = plan.traversal.last().unwrap().rail;
                // In this scenario, we replaced or deleted our end rail, we don't have a
                // way to resolve rail from pos and there could be overlapping
                // rails at the destination so we just abort the plan
                if rails.contains(to_rail) {
                    train.plan = graph.get_path(
                        train.t,
                        train.rail,
                        &t.forward(),
                        &intersections.intersections.get(&next.0).unwrap(),
                        &next.1,
                        plan.traversal.last().unwrap().rail,
                        &plan.end_position,
                        &intersections,
                        &rails,
                    );
                } else {
                    train.plan = None;
                }
            }
        }

        // Execute plan
        if let Some(plan) = &train.plan {
            let distance = 10.0 * time.delta_secs();
            let remaining_distance = if train.rail == plan.traversal.last().unwrap().rail {
                plan.end_position.distance(t.translation)
            } else {
                distance
            };

            let result = train.traverse(
                distance.min(remaining_distance),
                train.t,
                t.forward(),
                train.rail,
                rail,
                spline,
                &rails,
                &intersections,
                &plan,
            );

            // let delta = result.pos - t.translation;
            // for i in 0..200 {
            //     let i = i as f32;
            //     let pos = t.translation + delta * i;
            //     gizmos.sphere(Isometry3d::from_translation(pos.clone()), 0.3, GREEN_500);
            // }

            if result.status == TrainTraverseStatus::InTransit {
                let points = plan.points(&t.translation, &plan.end_position, train.rail, &rails);
                points.iter().zip(points.iter().skip(1)).for_each(|(x, y)| {
                    gizmos
                        .arrow(*x, *y, Color::srgb(0.5, 0.5, 0.5))
                        .with_tip_length(0.5);
                });
            } else {
                train.plan = None;
            }

            train.rail = result.rail;
            train.t = result.t;
            t.translation = result.pos;
            let up = t.up();
            t.look_at(result.pos + result.forward.as_vec3(), up);
        }
    });
}

fn handle_train_placement(
    mut c: Commands,
    mut q: Query<(&mut PlayerCursor, &ActionState<PlayerBuildAction>)>,
    mut preview: Query<(&mut Transform, &mut PlaceablePreview)>,
    rails: Query<&Spline, With<Rail>>,
    mut ray_cast: MeshRayCast,
    train: Res<TrainAsset>,
    mut feedback: ResMut<CursorFeedback>,
) {
    if preview.is_empty() {
        return;
    }
    let mut preview = preview.single_mut();

    let (mut cursor, input) = q.single_mut();
    let mut pos = cursor.build_pos;
    let mut spline_forward = preview.0.forward();
    let mut target_rail = None;
    let mut target_spline = None;

    let hit = get_closest_rail(cursor.ray, &mut ray_cast, &rails);
    if let Some(hit) = &hit {
        if let Ok(spline) = rails.get(hit.0) {
            let t = spline.t_from_pos(&pos);
            pos = spline.projected_position(t);
            spline_forward = spline.forward(t);
            cursor.manual_rotation = 0.0;

            let mut align_to_right = spline_forward.dot(preview.0.forward().as_vec3()) > 0.;
            if input.just_pressed(&PlayerBuildAction::Rotate) {
                align_to_right = !align_to_right;
            }

            spline_forward = if align_to_right {
                spline_forward
            } else {
                Dir3::new(spline_forward.as_vec3() * -1.0).unwrap()
            };

            target_rail = Some(hit.0);
            target_spline = Some(spline);
        }
    } else {
        spline_forward = Quat::from_rotation_y(cursor.manual_rotation) * spline_forward;
    }

    preview.0.translation = pos;
    preview.0.look_at(pos + spline_forward.as_vec3(), Vec3::Y);

    // TODO: Overlap check for other trains
    let collide_with_other = false;
    // let collide_with_other = hit.is_some() && Collider::int
    //     && spatial_query
    //         .shape_intersections(
    //             &train.collider,
    //             preview.0.translation,
    //             preview.0.rotation,
    //             &SpatialQueryFilter::default(),
    //         )
    //         .len()
    //         == 0;

    preview.1.valid = false;
    if hit.is_none() {
        feedback
            .entries
            .push(CursorFeedbackData::default().with_error("Not on rail".to_owned()));
    } else if collide_with_other {
        feedback
            .entries
            .push(CursorFeedbackData::default().with_error("Collide with other train".to_owned()));
    } else {
        preview.1.valid = true;
    }

    if preview.1.valid && input.just_pressed(&PlayerBuildAction::Interact) {
        c.spawn((
            Train {
                t: target_spline.unwrap().t_from_pos(&pos),
                rail: target_rail.unwrap(),
                plan: None,
            },
            preview.0.clone().with_scale(train.scale),
            SceneRoot(train.scene.clone()),
        ))
        .observe(on_destroy_default);
    }
}

fn calculate_plan(
    mut q: Query<(&Transform, &mut Train), With<Selected>>,
    mut rails: Query<(&Rail, &Spline)>,
    graph: Res<RailGraph>,
    player: Single<(&PlayerCursor, &ActionState<PlayerViewAction>)>,
    mut ray_cast: MeshRayCast,
    intersections: Res<RailIntersections>,
    mut gizmos: Gizmos,
    mut ev: EventWriter<SelectedChangedEvent>,
) {
    if q.is_empty() {
        return;
    }

    let (cursor, input) = player.into_inner();

    let end = get_closest_rail(
        cursor.ray,
        &mut ray_cast,
        &rails
            .transmute_lens_filtered::<&Spline, With<Rail>>()
            .query(),
    );
    if end.is_none() {
        return;
    }
    let end = end.unwrap();

    for (t, mut train) in q.iter_mut() {
        let (start, spline) = rails.get(train.rail).unwrap();
        let next = train.next_intersection(&t, start, spline);

        let plan = graph.get_path(
            train.t,
            train.rail,
            &t.forward(),
            intersections.intersections.get(&next.0).unwrap(),
            &next.1,
            end.0,
            &end.1.point,
            &intersections,
            &rails,
        );

        if let Some(plan) = plan {
            let points = plan.points(&t.translation, &plan.end_position, train.rail, &rails);
            points.iter().zip(points.iter().skip(1)).for_each(|(x, y)| {
                gizmos.arrow(*x, *y, Color::WHITE).with_tip_length(0.5);
            });

            if input.just_pressed(&PlayerViewAction::Interact) {
                train.plan = Some(plan);
                ev.send(SelectedChangedEvent(None));
            }
        }
    }
}
