use avian3d::prelude::*;
use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;
use serde::Deserialize;

#[cfg(feature = "dev")]
mod dev;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default())
            .init_asset::<DemoBoxConfig>()
            .register_asset_loader(DemoBoxConfigLoader)
            .add_systems(Startup, setup)
            // simulation runs on fixed ticks (heuristics.toml: sim-in-fixed-update)
            .add_systems(FixedUpdate, move_box);

        #[cfg(feature = "dev")]
        app.add_plugins(dev::DevToolsPlugin);
    }
}

/// Marker for the demo box — query gameplay entities by marker, never by
/// rendering components like `Mesh3d`.
#[derive(Component)]
pub struct DemoBox;

/// Marker for the demo ball — a dynamic body the kinematic box shoves around,
/// proving physics is live (gravity, collision, restitution).
#[derive(Component)]
pub struct DemoBall;

/// Tunables for the demo box, hot-reloaded from `assets/config/demo_box.ron`:
/// edit the file while `just run` and the box changes instantly — no rebuild
/// (heuristics.toml: no-magic-numbers).
#[derive(Asset, TypePath, Deserialize, Clone, Copy)]
pub struct DemoBoxConfig {
    pub speed: f32,
    pub range: f32,
}

impl Default for DemoBoxConfig {
    // fallback while the asset is still loading (and in headless tests)
    fn default() -> Self {
        Self {
            speed: 2.0,
            range: 4.0,
        }
    }
}

/// Handle to the loaded config — systems resolve it against `Assets` each
/// frame, which is what makes hot reload free: the watcher swaps the asset,
/// the next lookup sees new values.
#[derive(Resource)]
pub struct DemoBoxConfigHandle(pub Handle<DemoBoxConfig>);

#[derive(TypePath)]
struct DemoBoxConfigLoader;

impl AssetLoader for DemoBoxConfigLoader {
    type Asset = DemoBoxConfig;
    type Settings = ();
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(ron::de::from_bytes(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

// Scene layout (fixed geometry, not gameplay tunables — those live in RON)
const GROUND_SIZE: f32 = 20.0;
const GROUND_THICKNESS: f32 = 0.2;
const BOX_SIZE: f32 = 1.0;
const BALL_RADIUS: f32 = 0.5;
const BALL_DROP_POS: Vec3 = Vec3::new(1.5, 4.0, 0.0);

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(DemoBoxConfigHandle(
        asset_server.load("config/demo_box.ron"),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 6.0, 14.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(GROUND_SIZE, GROUND_THICKNESS, GROUND_SIZE),
        Mesh3d(meshes.add(Cuboid::new(GROUND_SIZE, GROUND_THICKNESS, GROUND_SIZE))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
        Transform::from_xyz(0.0, -GROUND_THICKNESS / 2.0, 0.0),
    ));

    // Kinematic: the sim (`move_box`) drives it by velocity; avian makes it a
    // solid obstacle that shoves dynamic bodies without being pushed back.
    // Asset hot reload playground: overwrite assets/images/box.png while
    // running (e.g. `just swap-asset`) and the texture updates live.
    commands.spawn((
        DemoBox,
        RigidBody::Kinematic,
        Collider::cuboid(BOX_SIZE, BOX_SIZE, BOX_SIZE),
        Mesh3d(meshes.add(Cuboid::from_length(BOX_SIZE))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load("images/box.png")),
            ..default()
        })),
        Transform::from_xyz(0.0, BOX_SIZE / 2.0, 0.0),
    ));

    // Dynamic: gravity drops it onto the ground, the oscillating box knocks
    // it around — physics visibly live the moment the app opens.
    commands.spawn((
        DemoBall,
        RigidBody::Dynamic,
        Collider::sphere(BALL_RADIUS),
        Restitution::new(0.7),
        Mesh3d(meshes.add(Sphere::new(BALL_RADIUS))),
        MeshMaterial3d(materials.add(Color::srgb(0.9, 0.3, 0.3))),
        Transform::from_translation(BALL_DROP_POS),
    ));
}

fn move_box(
    time: Res<Time>,
    handle: Option<Res<DemoBoxConfigHandle>>,
    configs: Option<Res<Assets<DemoBoxConfig>>>,
    mut boxes: Query<(&Transform, &mut LinearVelocity), With<DemoBox>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return; // first frame: no time has passed, no velocity to command
    }
    // config not registered/loaded yet -> defaults; loaded -> live RON values
    let config = match (&handle, &configs) {
        (Some(handle), Some(configs)) => configs.get(&handle.0).copied().unwrap_or_default(),
        _ => DemoBoxConfig::default(),
    };
    // Kinematic bodies must be driven by velocity, not Transform writes: the
    // solver needs a real contact velocity to shove dynamic bodies (a
    // teleported box has velocity zero and grinds through the ball). Command
    // the velocity that lands exactly on the analytic path this tick — moved
    // by the solver, but zero integration drift.
    for (transform, mut velocity) in &mut boxes {
        let target_x = box_x(time.elapsed_secs(), config.speed, config.range);
        velocity.x = (target_x - transform.translation.x) / dt;
    }
}

/// Horizontal position of the demo box: oscillates ±`range` around 0 over time.
///
/// Pure function — the doc example below is compiled and run by `just test`
/// (`cargo test --doc`), and rust-analyzer shows it on hover in the editor.
///
/// ```
/// use bevy_dx::box_x;
///
/// // at t = 0 the box is centered, regardless of speed/range
/// assert_eq!(box_x(0.0, 2.0, 4.0), 0.0);
/// // never leaves ±range
/// assert!(box_x(1.234, 2.0, 4.0).abs() <= 4.0);
/// ```
#[must_use]
pub fn box_x(elapsed_secs: f32, speed: f32, range: f32) -> f32 {
    (elapsed_secs * speed).sin() * range
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plugin is the unit: exercise `GamePlugin` through a headless App —
    /// wiring bugs surface as behavior failures, never as config assertions.
    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_plugins(GamePlugin);
        // `App::run` normally does this; headless tests drive `update` directly.
        // avian registers resources in `Plugin::finish`, so skipping it panics.
        app.finish();
        app.cleanup();
        app
    }

    #[test]
    fn game_plugin_spawns_exactly_one_demo_box() {
        let mut app = headless_app();
        app.update();

        let mut boxes = app.world_mut().query_filtered::<(), With<DemoBox>>();
        assert_eq!(boxes.iter(app.world()).count(), 1);
    }

    #[test]
    fn demo_box_oscillates_on_fixed_ticks() {
        let mut app = headless_app();
        app.update();
        // the sim runs in FixedUpdate (64 Hz): sleep past at least one fixed tick
        std::thread::sleep(std::time::Duration::from_millis(40));
        app.update();

        let fixed_elapsed = app.world().resource::<Time<Fixed>>().elapsed_secs();
        assert!(fixed_elapsed > 0.0, "no fixed tick ran");
        let mut boxes = app
            .world_mut()
            .query_filtered::<&Transform, With<DemoBox>>();
        let x = boxes.single(app.world()).unwrap().translation.x;
        let defaults = DemoBoxConfig::default();
        // the solver integrates the commanded velocity, so the box lands on
        // the analytic path within float error, not bit-exactly
        let expected = box_x(fixed_elapsed, defaults.speed, defaults.range);
        assert!(
            (x - expected).abs() < 1e-3,
            "box at {x}, expected ~{expected}"
        );
    }

    /// Shipped data must parse: a broken RON falls back to defaults silently
    /// at runtime, so parseability is only caught here.
    #[test]
    fn shipped_config_ron_parses() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/config");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            ron::de::from_bytes::<DemoBoxConfig>(&bytes)
                .unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
        }
    }

    /// Unit scope: `move_box` is private — only in-module tests can reach it.
    /// Headless pattern: `MinimalPlugins` (no window/renderer) + spawn + update + assert.
    /// The system is scheduled in `Update` here to drive it directly; the
    /// plugin's `FixedUpdate` scheduling is covered by the integration tests.
    #[test]
    #[allow(clippy::float_cmp)] // exact: same computation both sides
    fn move_box_commands_velocity_toward_analytic_path() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, move_box);
        let id = app
            .world_mut()
            .spawn((DemoBox, Transform::default(), LinearVelocity::default()))
            .id();

        app.update(); // first tick: delta is zero
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update(); // second tick: time has advanced

        let time = app.world().resource::<Time>();
        let (elapsed, dt) = (time.elapsed_secs(), time.delta_secs());
        assert!(dt > 0.0);
        let vel = app.world().get::<LinearVelocity>(id).unwrap().x;
        let defaults = DemoBoxConfig::default();
        // box starts at x=0, so the commanded velocity covers the full
        // distance to the analytic target within one tick
        assert_eq!(vel, box_x(elapsed, defaults.speed, defaults.range) / dt);
    }

    /// Entities without the `DemoBox` marker must be left alone.
    #[test]
    #[allow(clippy::float_cmp)] // exact: untouched value compared to its initializer
    fn move_box_ignores_unmarked_entities() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, move_box);
        let id = app
            .world_mut()
            .spawn((Transform::default(), LinearVelocity::default()))
            .id();

        app.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update();

        assert_eq!(app.world().get::<LinearVelocity>(id).unwrap().x, 0.0);
    }
}
