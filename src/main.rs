use amethyst::{
    assets::{AssetStorage, Loader},
    core::{
        bundle::SystemBundle,
        frame_limiter::FrameRateLimitStrategy,
        timing::Time,
        transform::{Transform, TransformBundle},
    },
    ecs::{
        prelude::DispatcherBuilder, Component, DenseVecStorage, Join, Read, ReadStorage, System,
        WriteStorage,
    },
    error::Error,
    prelude::*,
    renderer::{
        Camera, DisplayConfig, DrawFlat2D, Pipeline, PngFormat, Projection, RenderBundle,
        SpriteRender, SpriteSheet, SpriteSheetFormat, SpriteSheetHandle, Stage, Texture,
        TextureMetadata,
    },
    utils::application_root_dir,
};
use rand::Rng;
use std::time::Duration;

const ARENA_WIDTH: f32 = 1000.0;
const ARENA_HEIGHT: f32 = 1000.0;

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let root = application_root_dir()?;

    let config_path = root.join("resources").join("display_config.ron");

    let config = DisplayConfig::load(&config_path);

    let pipe = Pipeline::build().with_stage(
        Stage::with_backbuffer()
            .clear_target([0.0, 0.0, 0.0, 1.0], 1.0)
            .with_pass(DrawFlat2D::new()),
    );

    let game_data = GameDataBuilder::default()
        .with_bundle(BounceBundle)?
        .with_bundle(RenderBundle::new(pipe, Some(config)).with_sprite_sheet_processor())?
        .with_bundle(TransformBundle::new())?;

    let mut game = Application::build(root, State)?
        .with_frame_limit(
            FrameRateLimitStrategy::SleepAndYield(Duration::from_millis(2)),
            144,
        )
        .build(game_data)?;

    game.run();

    Ok(())
}

struct BounceBundle;

impl<'a, 'b> SystemBundle<'a, 'b> for BounceBundle {
    fn build(self, builder: &mut DispatcherBuilder<'a, 'b>) -> Result<(), Error> {
        builder.add(MovementSystem, "movement_system", &[]);
        builder.add(BounceSystem, "bounce_system", &[]);

        Ok(())
    }
}

struct State;

impl SimpleState for State {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        let world = data.world;

        let mut camera_transform = Transform::default();
        camera_transform.set_translation_z(1.0);

        world
            .create_entity()
            .with(Camera::from(Projection::orthographic(
                0.0,
                ARENA_WIDTH,
                0.0,
                ARENA_HEIGHT,
            )))
            .with(camera_transform)
            .build();

        let sprite_sheet_handle = load_sprite_sheet(world);

        let mut rng = rand::thread_rng();

        for _ in 0..100_000 {
            let mut ball_transform = Transform::default();
            let x = ARENA_WIDTH / 2.0;
            let y = ARENA_HEIGHT / 2.0;

            ball_transform.set_translation_xyz(x, y, 0.);

            let sprite_sheet = SpriteRender {
                sprite_sheet: sprite_sheet_handle.clone(),
                sprite_number: 0,
            };

            let range = 50.0;

            world
                .create_entity()
                .with(sprite_sheet)
                .with(Velocity {
                    x: rng.gen_range(-range, range),
                    y: rng.gen_range(-range, range),
                })
                .with(ball_transform)
                .build();
        }
    }
}

fn load_sprite_sheet(world: &mut World) -> SpriteSheetHandle {
    let texture_handle = {
        let loader = world.read_resource::<Loader>();
        let texture_storage = world.read_resource::<AssetStorage<Texture>>();

        loader.load(
            "assets/spritesheet.png",
            PngFormat,
            TextureMetadata::srgb_scale(),
            (),
            &texture_storage,
        )
    };

    let loader = world.read_resource::<Loader>();
    let sprite_sheet_store = world.read_resource::<AssetStorage<SpriteSheet>>();

    loader.load(
        "resources/spritesheet.ron",
        SpriteSheetFormat,
        texture_handle,
        (),
        &sprite_sheet_store,
    )
}

struct MovementSystem;

impl<'s> System<'s> for MovementSystem {
    type SystemData = (
        WriteStorage<'s, Transform>,
        ReadStorage<'s, Velocity>,
        Read<'s, Time>,
    );

    fn run(&mut self, (mut transforms, velocities, time): Self::SystemData) {
        for (transform, velocity) in (&mut transforms, &velocities).join() {
            transform.prepend_translation_x(velocity.x * time.delta_seconds());
            transform.prepend_translation_y(velocity.y * time.delta_seconds());
        }
    }
}

struct BounceSystem;

impl<'s> System<'s> for BounceSystem {
    type SystemData = (WriteStorage<'s, Velocity>, ReadStorage<'s, Transform>);

    fn run(&mut self, (mut velocities, transforms): Self::SystemData) {
        for (velocity, transform) in (&mut velocities, &transforms).join() {
            let transform_pos = transform.translation();

            if transform_pos.y >= ARENA_HEIGHT || transform_pos.y <= 0.0 {
                velocity.y = -velocity.y;
            }

            if transform_pos.x >= ARENA_WIDTH || transform_pos.x <= 0.0 {
                velocity.x = -velocity.x;
            }
        }
    }
}

struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl Component for Velocity {
    type Storage = DenseVecStorage<Self>;
}
