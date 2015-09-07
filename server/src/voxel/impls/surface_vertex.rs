use cgmath::{Point, Point3, Vector, EuclideanVector, Vector3};
use std::cmp::{min, max};
use std::f32;
use std::ops::Neg;
use stopwatch;

use voxel;

// NOTE: When voxel size and storage become an issue, this should be shrunk to
// be less than pointer-sized. It'll be easier to transfer to the GPU for
// whatever reasons, but also make it possible to shrink the SVO footprint by
// "flattening" the leaf contents and pointers into the same space (the
// low-order bits can be used to figure out which one it is, since pointers
// have three low-order bits set to zero).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum T<Material> {
  // The entire voxel is a single material.
  Volume(Material),
  // The voxel crosses the surface of the volume.
  Surface(SurfaceStruct<Material>),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
// Every voxel keeps track of a single vertex, as well as whether its
// lowest-coordinate corner is inside the volume.
// Since we keep track of an "arbitrarily" large world of voxels, we don't
// leave out any corners.
pub struct SurfaceStruct<Material> {
  /// The position of a free-floating vertex on the surface.
  pub surface_vertex: Vertex,
  /// The surface normal at `surface_vertex`.
  pub normal: Normal,

  /// The material of the voxel's lowest corner.
  pub corner: Material,
}

// TODO: Should this be moved into the general voxel interface?
pub fn of_field<Field>(
  field: &Field,
  voxel: &::voxel::Bounds,
) -> T<Option<::voxel::Material>> where
  Field: voxel::mosaic::T,
{
  stopwatch::time("voxel.surface_vertex.of_field", || {
    let (low, high) = voxel.corners();
    macro_rules! material_at(($x:expr, $y:expr, $z:expr) => {{
      let p = Point3::new($x.x, $y.y, $z.z);
      debug!("Finding material at {:?}", p);
      voxel::mosaic::T::material(field, &p)
    }});

    // TODO: Re=evaluating the density function is costly. Do it only once for each corner.

    let corner = material_at!(low, low, low);
    let is_homogenous = || {
      macro_rules! check_corner(($x:expr, $y:expr, $z:expr) => {{
        let material = material_at!($x, $y, $z);
        debug!("material is {:?}", material);
        if material != corner {
          return false
        }
      }});
      check_corner!( low,  low, high);
      check_corner!( low, high,  low);
      check_corner!( low, high, high);
      check_corner!(high,  low,  low);
      check_corner!(high,  low, high);
      check_corner!(high, high,  low);
      check_corner!(high, high, high);
      return true
    };
    let is_homogenous = is_homogenous();
    debug!("is_homogenous: {:?}", is_homogenous);

    if is_homogenous {
      return T::Volume(corner)
    }

    let corner_coords = [0.0, 256.0];
    let mut total_weight = 0.0;
    macro_rules! weighted(($x:expr, $y:expr, $z:expr) => {{
      let x = if $x == 0 { low.x } else { high.x };
      let y = if $y == 0 { low.y } else { high.y };
      let z = if $z == 0 { low.z } else { high.z };
      let corner = voxel::mosaic::T::density(field, &Point3::new(x, y, z));
      assert!(corner >= 0.0);
      let corner = 1.0 / (corner + f32::EPSILON);
      total_weight += corner;
      Vector3::new(corner_coords[$x], corner_coords[$y], corner_coords[$z]).mul_s(corner)
    }});

    let vertex =
      weighted!(0, 0, 0) +
      weighted!(0, 0, 1) +
      weighted!(0, 1, 0) +
      weighted!(0, 1, 1) +
      weighted!(1, 0, 0) +
      weighted!(1, 0, 1) +
      weighted!(1, 1, 0) +
      weighted!(1, 1, 1) +
      Vector3::new(0.0, 0.0, 0.0);
    let vertex = Point3::from_vec(&vertex.div_s(total_weight));
    let vertex =
      Vertex {
        x: Fracu8::of(f32::min(vertex.x, 255.0) as u8),
        y: Fracu8::of(f32::min(vertex.y, 255.0) as u8),
        z: Fracu8::of(f32::min(vertex.z, 255.0) as u8),
      };

    let normal;
    {
      // Okay, this is silly to have right after we construct the vertex.
      let vertex = vertex.to_world_vertex(voxel);
      normal = Normal::of_float_normal(&voxel::field::T::normal(field, &vertex));
    }

    T::Surface(SurfaceStruct {
      surface_vertex: vertex,
      normal: normal,
      corner: corner,
    })
  })
}

pub fn unwrap<X>(voxel: T<Option<X>>) -> T<X> {
  match voxel {
    T::Volume(x) => T::Volume(x.unwrap()),
    T::Surface(x) => {
      T::Surface(SurfaceStruct {
        surface_vertex: x.surface_vertex,
        normal: x.normal,
        corner: x.corner.unwrap(),
      })
    }
  }
}

impl voxel::T for T<::voxel::Material> {
  fn brush<Mosaic>(
    this: &mut T<::voxel::Material>,
    bounds: &voxel::Bounds,
    brush: &voxel::brush::T<Mosaic>,
  ) where Mosaic: voxel::mosaic::T
  {
    let set_leaf = |this: &mut T<::voxel::Material>, corner| {
      debug!("leaf {:?} is {:?}", bounds, *this);
      match of_field(&brush.mosaic, bounds) {
        T::Volume(None) => {}
        T::Volume(Some(material)) => {
          *this = T::Volume(material);
          debug!("leaf {:?} set to {:?}", bounds, *this);
        },
        T::Surface(surface) => {
          let size = bounds.size();
          let low = Point3::new(bounds.x as f32, bounds.y as f32, bounds.z as f32);
          let low = low.mul_s(size);
          let corner =
            match voxel::mosaic::T::material(&brush.mosaic, &low) {
              None => corner,
              Some(material) => material,
            };
          let voxel =
            SurfaceStruct {
              surface_vertex: surface.surface_vertex,
              normal: surface.normal,
              corner: corner,
            };
          *this = T::Surface(voxel);
          debug!("leaf {:?} set to {:?}", bounds, *this);
        },
      }
    };

    match this {
      &mut T::Volume(material) => {
        set_leaf(this, material);
      },
      &mut T::Surface(voxel) => {
        set_leaf(this, voxel.corner);
      },
    }
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Vertex {
  pub x: Fracu8,
  pub y: Fracu8,
  pub z: Fracu8,
}

impl Vertex {
  #[allow(dead_code)]
  pub fn of_world_vertex_in(vertex: &Point3<f32>, voxel: &voxel::Bounds) -> Vertex {
    assert!(voxel.contains(vertex), "{:?} does not contain {:?}", voxel, vertex);

    let local = vertex.sub_p(&voxel.low_corner());
    let local = local.mul_s(256.0);
    let local =
      Vector3::new(
        local.x as u32 >> voxel.lg_size,
        local.y as u32 >> voxel.lg_size,
        local.z as u32 >> voxel.lg_size,
      );
    Vertex {
      x: Fracu8::of(min(0, max(255, local.x as u8))),
      y: Fracu8::of(min(0, max(255, local.y as u8))),
      z: Fracu8::of(min(0, max(255, local.z as u8))),
    }
  }

  pub fn to_world_vertex(&self, parent: &voxel::Bounds) -> Point3<f32> {
    // Relative position of the vertex.
    let local =
      Vector3::new(
        self.x.numerator as f32,
        self.y.numerator as f32,
        self.z.numerator as f32,
      )
      .div_s(256.0)
    ;
    let fparent = Point3::new(parent.x as f32, parent.y as f32, parent.z as f32);
    fparent.add_v(&local).mul_s(parent.size())
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Normal {
  pub x: Fraci8,
  pub y: Fraci8,
  pub z: Fraci8,
}

impl Normal {
  pub fn of_float_normal(normal: &Vector3<f32>) -> Normal {
    // Okay, so we scale the normal by 127, and use 127 to represent 1.0.
    // Then we store it in a `Fraci8`, which scales by 128 and represents a
    // fraction in [-1,1). That seems wrong, but this is normal data, so scaling
    // doesn't matter. Sketch factor is over 9000, but it's not wrong.

    let normal = normal.mul_s(127.0);
    let normal = Vector3::new(normal.x as i32, normal.y as i32, normal.z as i32);
    Normal {
      x: Fraci8::of(max(-127, min(127, normal.x)) as i8),
      y: Fraci8::of(max(-127, min(127, normal.y)) as i8),
      z: Fraci8::of(max(-127, min(127, normal.z)) as i8),
    }
  }

  pub fn to_float_normal(&self) -> Vector3<f32> {
    Vector3::new(self.x.to_f32(), self.y.to_f32(), self.z.to_f32()).normalize()
  }
}

impl Neg for Normal {
  type Output = Normal;

  fn neg(self) -> Normal {
    Normal {
      x: Fraci8::of(-max(-127, self.x.numerator)),
      y: Fraci8::of(-max(-127, self.y.numerator)),
      z: Fraci8::of(-max(-127, self.z.numerator)),
    }
  }
}

/// Express a `[0,1)` fraction using a `u8`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Fracu8 {
  // The denominator is 1 << 8.
  pub numerator: u8,
}

impl Fracu8 {
  pub fn of(numerator: u8) -> Fracu8 {
    Fracu8 {
      numerator: numerator,
    }
  }
}

/// Express a `[-1,1)` fraction using a `i8`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Fraci8 {
  // The denominator is 1 << 8.
  pub numerator: i8,
}

impl Fraci8 {
  pub fn of(numerator: i8) -> Fraci8 {
    Fraci8 {
      numerator: numerator,
    }
  }

  pub fn to_f32(&self) -> f32 {
    self.numerator as f32 / 128.0
  }
}
