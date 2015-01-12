use color::Color4;
use common::*;
use gl::types::*;
use mob;
use nalgebra::Vec3;
use physics::Physics;
use state::App;
use std::ops::{Deref, DerefMut};
use terrain_block::BlockPosition;
use yaglw::gl_context::GLContext;

macro_rules! translate_mob(
  ($world:expr, $mob:expr, $v:expr) => (
    translate_mob(
      $world.gl_context,
      &mut $world.physics,
      &mut $world.mob_buffers,
      $mob,
      $v
    );
  );
);

pub fn update<'a>(app: &mut App) {
  app.timers.time("update", || {
    app.timers.time("update.player", || {
      app.player.update(
        app.timers,
        app.gl_context,
        &mut app.terrain_game_loader,
        &mut app.id_allocator,
        &mut app.physics,
      );
    });

    app.timers.time("update.mobs", || {
      for (_, mob) in app.mobs.iter() {
        let mut mob_cell = mob.deref().borrow_mut();
        let mob = mob_cell.deref_mut();

        let block_position = BlockPosition::from_world_position(&mob.position);

        mob.solid_boundary.update(
          app.timers,
          app.gl_context,
          &mut app.terrain_game_loader,
          &mut app.id_allocator,
          &mut app.physics,
          block_position,
        );

        {
          let behavior = mob.behavior;
          (behavior)(app, mob);
        }

        mob.speed = mob.speed - Vec3::new(0.0, 0.1, 0.0 as GLfloat);

        let delta_p = mob.speed;
        if delta_p.x != 0.0 {
          translate_mob!(app, mob, Vec3::new(delta_p.x, 0.0, 0.0));
        }
        if delta_p.y != 0.0 {
          translate_mob!(app, mob, Vec3::new(0.0, delta_p.y, 0.0));
        }
        if delta_p.z != 0.0 {
          translate_mob!(app, mob, Vec3::new(0.0, 0.0, delta_p.z));
        }
      }
    });
  })
}

fn translate_mob(
  gl: &mut GLContext,
  physics: &mut Physics,
  mob_buffers: &mut mob::MobBuffers,
  mob: &mut mob::Mob,
  delta_p: Vec3<GLfloat>,
) {
  if physics.translate_misc(mob.id, delta_p).is_some() {
    mob.speed = mob.speed - delta_p;
  } else {
    let bounds = physics.get_bounds(mob.id).unwrap();
    mob.position = mob.position + delta_p;
    mob_buffers.update(
      gl,
      mob.id,
      &to_triangles(bounds, &Color4::of_rgba(1.0, 0.0, 0.0, 1.0))
    );
  }
}
