use cgmath;
use collision;
use common::surroundings_loader;
use common::surroundings_loader::LoadType;
use common::voxel;
use client_lib::{chunk, lod, terrain_mesh};
use server_lib::{server, update_gaia};
use server_lib::update_gaia::LoadDestination;

pub fn max_load_distance() -> u32 {
  80
}

pub fn player_position() -> cgmath::Point3<f32> {
  cgmath::Point3::new(0.0, 512.0, 0.0)
}

pub fn generate_all_terrain() -> Vec<Vec<(voxel::bounds::T, voxel::T)>> {
  let mut r = Vec::new();

  let server = server::new();

  let load_position = chunk::position::of_world_position(&player_position());

  let mut surroundings_loader = {
    surroundings_loader::new(
      max_load_distance(),
      lod::THRESHOLDS.iter().map(|&x| x as i32).collect(),
    )
  };
  let mut updates = surroundings_loader.updates(load_position.as_pnt());

  loop {
    let chunk_position;
    let load_type;
    match updates.next() {
      None => break,
      Some((b, l)) => {
        chunk_position = chunk::position::of_pnt(&b);
        load_type = l;
      },
    }

    debug!("chunk {:?}", chunk_position);
    let distance =
      surroundings_loader::distance_between(
        load_position.as_pnt(),
        chunk_position.as_pnt(),
      );

    let lod;
    match load_type {
      LoadType::Load => {
        lod = lod::of_distance(distance as u32);
      },
      LoadType::Downgrade => {
        panic!("Downgrading should not happen");
      },
      LoadType::Unload => {
        panic!("Unloading should not happen");
      },
    };

    let voxel_size = 1 << lod.lg_sample_size();
    let voxels =
      terrain_mesh::voxels_in(
        &collision::Aabb3::new(
          cgmath::Point3::new(
            (chunk_position.as_pnt().x << chunk::LG_WIDTH) - voxel_size,
            (chunk_position.as_pnt().y << chunk::LG_WIDTH) - voxel_size,
            (chunk_position.as_pnt().z << chunk::LG_WIDTH) - voxel_size,
          ),
          cgmath::Point3::new(
            ((chunk_position.as_pnt().x + 1) << chunk::LG_WIDTH) + voxel_size,
            ((chunk_position.as_pnt().y + 1) << chunk::LG_WIDTH) + voxel_size,
            ((chunk_position.as_pnt().z + 1) << chunk::LG_WIDTH) + voxel_size,
          ),
        ),
        lod.lg_sample_size(),
      );

    update_gaia::update_gaia(&server, update_gaia::Message::Load(0, voxels, LoadDestination::Callback(Box::new(|v| r.push(v)))));
  }

  r
}
