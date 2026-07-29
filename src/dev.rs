//! Dev-only tooling — compiled solely behind the `dev` cargo feature, so none
//! of this exists in a release binary.

use avian3d::prelude::PhysicsDebugPlugin;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::input::common_conditions::input_toggle_active;
use bevy::prelude::*;
use bevy::remote::{RemotePlugin, http::RemoteHttpPlugin};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            EguiPlugin::default(),
            // live world inspector: browse/edit every entity, component, and
            // resource in the running game. Toggle with ` (backquote).
            WorldInspectorPlugin::default().run_if(input_toggle_active(false, KeyCode::Backquote)),
            FpsOverlayPlugin::default(),
            // BRP: the running game answers HTTP on 127.0.0.1:15702 —
            // query/mutate/spawn entities from outside the process
            // (agent playtesting, scripted probes, curl)
            RemotePlugin::default(),
            RemoteHttpPlugin::default(),
            // wireframe colliders/contacts over the real render
            PhysicsDebugPlugin,
        ));
    }
}
