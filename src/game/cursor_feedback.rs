//! In world feedback system, every frame will display latest set text
use super::*;

pub(super) fn cursor_feedback_plugin(app: &mut App) {
    app.init_resource::<CursorFeedback>();
    app.add_systems(Startup, spawn_build_planner_feedback);
    app.add_systems(PreUpdate, clear_build_planner_feedback_data);
    app.add_systems(PostUpdate, update_build_planner_feedback);
}

#[derive(Resource, Default)]
pub struct CursorFeedback {
    pub entries: Vec<CursorFeedbackData>,
}

#[derive(Default)]
pub struct CursorFeedbackData {
    pub status: String,
    pub error: String,
    pub duration: f32,
}

impl CursorFeedbackData {
    pub fn with_status(mut self, text: String) -> Self {
        self.status = text;
        self
    }

    pub fn with_error(mut self, text: String) -> Self {
        self.error = text;
        self
    }

    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration;
        self
    }
}

/// Text in world space representing the status
#[derive(Component)]
#[require(Text, TextFont(|| default_text_font()))]
struct CursorFeedbackText;

fn spawn_build_planner_feedback(mut c: Commands) {
    c.spawn(CursorFeedbackText);
}

fn clear_build_planner_feedback_data(mut feedback: ResMut<CursorFeedback>, time: Res<Time>) {
    feedback.entries.retain_mut(|x| {
        x.duration -= time.delta_secs();
        x.duration > 0.0
    });
}

fn update_build_planner_feedback(
    feedback: Single<(&mut Text, &mut Node), With<CursorFeedbackText>>,
    data: Res<CursorFeedback>,
    cursor: Single<&PlayerCursor>,
) {
    let (mut text, mut node) = feedback.into_inner();

    if let Some(data) = data.entries.last() {
        text.0 = String::new();
        text.0 += data.error.as_str();
        if text.0.is_empty() == false {
            text.0 += "\n";
        }
        text.0 += data.status.as_str();

        if let Some(pos) = cursor.screen_pos {
            node.left = Val::Px(pos.x);
            node.top = Val::Px(pos.y - 48.);
        }
    } else {
        text.0.clear();
    }
}
