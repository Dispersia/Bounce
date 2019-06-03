use std::sync::Arc;
use amethyst::{
    Application,
    assets::{AssetStorage, Loader, Processor},
    core::{
        bundle::SystemBundle,
        Float,
        frame_limiter::FrameRateLimitStrategy,
        timing::Time,
        transform::{Transform, TransformBundle},
    },
    ecs::{
        prelude::{DispatcherBuilder}, Component, DenseVecStorage, Join, Read, ReadExpect, Resources, ReadStorage, System, SystemData,
        WriteStorage,
    },
    error::Error,
    GameData,
    prelude::{Builder, GameDataBuilder, World},
    renderer::{
        camera::{Camera, Projection},
        formats::texture::ImageFormat,
        pass::DrawFlat2DDesc,
        rendy::{
            graph::{
                render::{RenderGroupDesc, SubpassBuilder},
                GraphBuilder
            },
            hal::{format::Format, image},
            factory::Factory,
        },
        sprite::{SpriteRender, SpriteSheet, SpriteSheetFormat, SpriteSheetHandle},
        RenderingSystem,
        GraphCreator,
        Texture,
        types::DefaultBackend,
    },
    SimpleState,
    StateData,
    utils::{
        application_root_dir,
    },
    window::{ScreenDimensions, WindowBundle, Window},
};
use rand::Rng;
use std::time::Duration;

const ARENA_WIDTH: f32 = 1000.0;
const ARENA_HEIGHT: f32 = 1000.0;

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let root = application_root_dir()?;

    let config_path = root.join("resources").join("display_config.ron");

    let game_data = GameDataBuilder::default()
        .with_bundle(BounceBundle)?
        .with_bundle(WindowBundle::from_config_path(config_path))?
        .with_bundle(TransformBundle::new())?
        .with_thread_local(RenderingSystem::<DefaultBackend, _>::new(ExampleGraph::default()));

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
        builder.add(Processor::<SpriteSheet>::new(), "sprite_sheet_processor", &[]);

        Ok(())
    }
}

#[derive(Default)]
struct ExampleGraph {
    dimensions: Option<ScreenDimensions>,
    surface_format: Option<Format>,
    dirty: bool,
}

impl GraphCreator<DefaultBackend> for ExampleGraph {
    fn rebuild(&mut self, res: &Resources) -> bool {
        // Rebuild when dimensions change, but wait until at least two frames have the same.
        let new_dimensions = res.try_fetch::<ScreenDimensions>();
        use std::ops::Deref;
        if self.dimensions.as_ref() != new_dimensions.as_ref().map(|d| d.deref()) {
            self.dirty = true;
            self.dimensions = new_dimensions.map(|d| d.clone());
            return false;
        }
        return self.dirty;
    }

    fn builder(
        &mut self,
        factory: &mut Factory<DefaultBackend>,
        res: &Resources,
    ) -> GraphBuilder<DefaultBackend, Resources> {
        use amethyst::renderer::rendy::{
            graph::present::PresentNode,
            hal::command::{ClearDepthStencil, ClearValue},
        };

        self.dirty = false;

        let window = <ReadExpect<'_, Arc<Window>>>::fetch(res);
        let surface = factory.create_surface(&window);
        // cache surface format to speed things up
        let surface_format = *self
            .surface_format
            .get_or_insert_with(|| factory.get_surface_format(&surface));

        let dimensions = self.dimensions.as_ref().unwrap();
        let window_kind =
            image::Kind::D2(dimensions.width() as u32, dimensions.height() as u32, 1, 1);

        let mut graph_builder = GraphBuilder::new();
        let color = graph_builder.create_image(
            window_kind,
            1,
            surface_format,
            Some(ClearValue::Color([0.0, 0.0, 0.0, 1.0].into())),
        );

        let depth = graph_builder.create_image(
            window_kind,
            1,
            Format::D32Sfloat,
            Some(ClearValue::DepthStencil(ClearDepthStencil(1.0, 0))),
        );

        let sprite = graph_builder.add_node(
            SubpassBuilder::new()
                .with_group(DrawFlat2DDesc::new().builder())
                .with_color(color)
                .with_depth_stencil(depth)
                .into_pass(),
        );

        let _present = graph_builder
            .add_node(PresentNode::builder(factory, surface, color).with_dependency(sprite));

        graph_builder
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
                0.,
                ARENA_WIDTH,
                0.,
                ARENA_HEIGHT,
                0.1,
                2000.0,
            )))
            .with(camera_transform)
            .build();

        let sprite_sheet_handle = load_sprite_sheet(world);

        let mut rng = rand::thread_rng();

        for _ in 0..50_000 {
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

struct BounceSystem;

impl<'s> System<'s> for BounceSystem {
    type SystemData = (
        WriteStorage<'s, Velocity>,
        WriteStorage<'s, Transform>
    );

    fn run(&mut self, (mut velocities, mut transforms): Self::SystemData) {
        for (mut velocity, transform) in (&mut velocities, &mut transforms).join() {
            let transform: &mut Transform = transform;

            let current_y = transform.translation().y;
            let current_x = transform.translation().x;

            if current_y >= Float::from(ARENA_HEIGHT) {
                transform.set_translation_y(ARENA_HEIGHT - 1.0);
                velocity.y = -velocity.y;
            }

            if current_y <= Float::from(0.0) {
                transform.set_translation_y(0.0);
                velocity.y = -velocity.y;
            }

            if current_x >= Float::from(ARENA_WIDTH) {
                transform.set_translation_x(ARENA_WIDTH - 1.0);
                velocity.x = -velocity.x;
            }

            if current_x <= Float::from(0.0) {
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
