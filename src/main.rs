use amethyst::{
    assets::{AssetStorage, Loader},
    core::{
        bundle::SystemBundle,
        frame_limiter::FrameRateLimitStrategy,
        timing::Time,
        transform::{Transform, TransformBundle},
    },
    ecs::{
        prelude::DispatcherBuilder, Component, DenseVecStorage, Join, Read, ReadExpect,
        ReadStorage, System, WriteStorage,
    },
    error::Error,
    prelude::{Builder, GameDataBuilder, World},
    renderer::{
        camera::{Camera, Projection},
        formats::texture::ImageFormat,
        plugins::{RenderFlat2D, RenderToWindow},
        sprite::{SpriteRender, SpriteSheet, SpriteSheetFormat, SpriteSheetHandle},
        types::DefaultBackend,
        RenderingBundle, Texture,
    },
    utils::application_root_dir,
    window::ScreenDimensions,
    Application, GameData, SimpleState, StateData,
};

use amethyst::prelude::WorldExt;
use rand::Rng;
use std::time::Duration;

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let root = application_root_dir()?;

    let config_path = root.join("resources").join("display_config.ron");

    let game_data = GameDataBuilder::default()
        .with_bundle(BounceBundle)?
        .with_bundle(TransformBundle::new())?
        .with_bundle(
            RenderingBundle::<DefaultBackend>::new()
                .with_plugin(
                    RenderToWindow::from_config_path(config_path).with_clear([0.0, 0.0, 0.0, 1.0]),
                )
                .with_plugin(RenderFlat2D::default()),
        )?;

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
    fn build(
        self,
        _world: &mut World,
        builder: &mut DispatcherBuilder<'a, 'b>,
    ) -> Result<(), Error> {
        builder.add(WindowResizeSystem::new(), "window_resize_system", &[]);
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

        let (width, height) = get_dimensions(world);

        world
            .create_entity()
            .with(Camera::from(Projection::orthographic(
                0., width, 0., -height, 0.1, 2000.0,
            )))
            .with(camera_transform)
            .build();

        let sprite_sheet_handle = load_sprite_sheet(world);

        let mut rng = rand::thread_rng();

        for _ in 0..100_000 {
            let mut ball_transform = Transform::default();
            let x = width / 2.0;
            let y = height / 2.0;

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

fn get_dimensions(world: &mut World) -> (f32, f32) {
    let screen_dimensions = world.read_resource::<ScreenDimensions>();

    (screen_dimensions.width(), screen_dimensions.height())
}

fn load_sprite_sheet(world: &mut World) -> SpriteSheetHandle {
    let texture_handle = {
        let loader = world.read_resource::<Loader>();
        let texture_storage = world.read_resource::<AssetStorage<Texture>>();

        loader.load(
            "assets/spritesheet.png",
            ImageFormat::default(),
            (),
            &texture_storage,
        )
    };

    let loader = world.read_resource::<Loader>();
    let sprite_sheet_store = world.read_resource::<AssetStorage<SpriteSheet>>();

    loader.load(
        "resources/spritesheet.ron",
        SpriteSheetFormat(texture_handle),
        (),
        &sprite_sheet_store,
    )
}

pub struct MovementSystem;

impl<'s> System<'s> for MovementSystem {
    type SystemData = (
        WriteStorage<'s, Transform>,
        ReadStorage<'s, Velocity>,
        Read<'s, Time>,
    );

    fn run(&mut self, (mut transforms, velocities, time): Self::SystemData) {
        for (transform, velocity) in (&mut transforms, &velocities).join() {
            let transform: &mut Transform = transform;
            let velocity: &Velocity = velocity;
            let delta_seconds = time.delta_seconds();

            transform.prepend_translation_x(velocity.x * delta_seconds);
            transform.prepend_translation_y(velocity.y * delta_seconds);
        }
    }
}

struct WindowResizeSystem {
    last_dimensions: ScreenDimensions,
}

impl WindowResizeSystem {
    pub fn new() -> Self {
        Self {
            last_dimensions: ScreenDimensions::new(0, 0, 0.0),
        }
    }
}

impl<'s> System<'s> for WindowResizeSystem {
    type SystemData = (ReadExpect<'s, ScreenDimensions>, WriteStorage<'s, Camera>);

    fn run(&mut self, (screen_dimensions, mut cameras): Self::SystemData) {
        if self.last_dimensions != *screen_dimensions {
            for camera in (&mut cameras).join() {
                if let Some(ortho) = camera.projection_mut().as_orthographic_mut() {
                    ortho.set_bottom_and_top(0., -screen_dimensions.height());
                    ortho.set_left_and_right(0., screen_dimensions.width());
                }
            }

            self.last_dimensions = screen_dimensions.clone();
        }
    }
}

struct BounceSystem;

impl<'s> System<'s> for BounceSystem {
    type SystemData = (
        ReadExpect<'s, ScreenDimensions>,
        WriteStorage<'s, Velocity>,
        WriteStorage<'s, Transform>,
    );

    fn run(&mut self, (screen, mut velocities, mut transforms): Self::SystemData) {
        for (mut velocity, transform) in (&mut velocities, &mut transforms).join() {
            let transform: &mut Transform = transform;

            let current_y = transform.translation().y;
            let current_x = transform.translation().x;

            let height = screen.height();
            let width = screen.width();

            if current_y >= height {
                transform.set_translation_y(height - 1.0);
                velocity.y = -velocity.y;
            }

            if current_y <= 0.0 {
                transform.set_translation_y(0.0);
                velocity.y = -velocity.y;
            }

            if current_x >= width {
                transform.set_translation_x(width - 1.0);
                velocity.x = -velocity.x;
            }

            if current_x <= 0.0 {
                transform.set_translation_x(0.0);
                velocity.x = -velocity.x;
            }
        }
    }
}

pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl Component for Velocity {
    type Storage = DenseVecStorage<Self>;
}
